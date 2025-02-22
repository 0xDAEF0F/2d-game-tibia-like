use anyhow::Result;
use egui_macroquad::macroquad;
use log::{debug, error, info};
use macroquad::{Window, prelude::*};
use my_mmo::client::tasks::{tcp_reader_task, udp_recv_task};
use my_mmo::client::{Cc, ChatMessage, ClientChannel, render_entity_name};
use my_mmo::client::{MmoContext, OtherPlayer, OtherPlayers, Player, make_egui};
use my_mmo::constants::*;
use my_mmo::sendable::SendableSync;
use my_mmo::server::Direction;
use my_mmo::tcp::{TcpClientMsg, TcpServerMsg};
use my_mmo::udp::UdpClientMsg;
use my_mmo::*;
use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tiled::{Loader, Map};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader, stdin};
use tokio::net::{TcpSocket, TcpStream, UdpSocket};
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};

#[tokio::main]
async fn main() -> Result<()> {
    MmoLogger::init("debug");

    let socket = UdpSocket::bind("0.0.0.0:0").await?;
    let socket = Arc::new(socket);
    socket.connect(SERVER_UDP_ADDR).await?;

    let tcp_socket = TcpSocket::new_v4()?;
    let mut stream = tcp_socket.connect(SERVER_TCP_ADDR.parse()?).await?;

    let init_player = request_new_session_from_server(&mut stream).await?;

    println!(
        "username: {} was accepted by the server.",
        init_player.username
    );

    info!("client connected to server at: {}", SERVER_TCP_ADDR);

    let (cc_tx, cc_rx) = mpsc::unbounded_channel::<ClientChannel>();

    // Spawn async UDP receive task
    let join_handle1 = udp_recv_task(socket.clone(), cc_tx.clone(), init_player.id);

    // UDP task is not supposed to finish
    tokio::spawn(async move {
        _ = tokio::join!(join_handle1);
        error!("UDP receive task failed");
        std::process::exit(1);
    });

    // Macroquad configuration and window
    let conf = Conf {
        window_title: "MMO Game".to_string(),
        high_dpi: true,
        ..Default::default()
    };
    let player = Player {
        id: init_player.id,
        username: init_player.username,
        level: init_player.level,
        hp: init_player.hp,
        max_hp: init_player.max_hp,
        frame: 0,
        request_id: 0,
        speed: BASE_MOVE_DELAY,
        curr_location: init_player.location,
        prev_location: init_player.location,
        last_move_timer: 0.0,
        direction: init_player.direction,
    };

    Window::from_config(conf, draw(socket, stream, cc_rx, cc_tx, player));

    Ok(())
}

async fn draw(
    socket: Arc<UdpSocket>,
    tcp_stream: TcpStream,
    mut cc_rx: UnboundedReceiver<ClientChannel>,
    cc_tx: UnboundedSender<ClientChannel>,
    mut player: Player,
) {
    prevent_quit();

    let (tcp_reader, tcp_writer) = tcp_stream.into_split();
    let tcp_writer = Arc::new(Mutex::new(tcp_writer));

    let tcp_writer_ = Arc::clone(&tcp_writer);
    tokio::spawn(async move {
        let jh = tcp_reader_task(tcp_reader, cc_tx.clone(), player.id);
        _ = tokio::join!(jh);
        let mut attempts_to_reconnect = 0;
        while attempts_to_reconnect <= MAX_CONNECTION_RETRIES {
            warn!("attempting re-connection to server (TCP).");
            let tcp_socket = TcpSocket::new_v4().unwrap();
            let server_addr: SocketAddr = SERVER_TCP_ADDR.parse().unwrap();
            let Ok(mut tcp_stream) = tcp_socket.connect(server_addr).await else {
                attempts_to_reconnect += 1;
                tokio::time::sleep(Duration::from_secs(5)).await;
                continue;
            };

            // reconnect to the server
            let reconnect_msg = TcpClientMsg::Reconnect(player.id);
            let reconnect_msg = bincode::serialize(&reconnect_msg).unwrap();
            tcp_stream.write_all(&reconnect_msg).await.unwrap();

            let (tcp_reader, tcp_writer_new) = tcp_stream.into_split();
            *tcp_writer_.lock().unwrap() = tcp_writer_new;
            let jh = tcp_reader_task(tcp_reader, cc_tx.clone(), player.id);
            _ = tokio::join!(jh);
        }
        info!("exiting program.");
        std::process::exit(1);
    });

    let mut other_players = OtherPlayers(HashMap::new());

    let map = {
        let mut loader = Loader::new();
        loader.load_tmx_map("assets/basic-map.tmx").unwrap()
    };

    // TODO: refactor
    let tilesheet = Tilesheet::from_tileset(map.tilesets()[0].clone());
    let objects_tilesheet_a = Tilesheet::from_tileset(map.tilesets()[1].clone());
    let objects_tilesheet_b = Tilesheet::from_tileset(map.tilesets()[2].clone());
    let player_tilesheet = Tilesheet::from_tileset(map.tilesets()[3].clone());

    let mut game_objects = GameObjects::new();
    let mut moving_object: Option<Location> = None;

    let mut fps_logger = FpsLogger::new();
    let mut ping_monitor = PingMonitor::new();

    let mut is_disconnected = false;

    let mut mmo_context = MmoContext {
        username: player.username.clone(),
        user_text: "".to_string(),
        user_chat: vec![],
        server_tcp_write_stream: tcp_writer.clone(),
    };

    loop {
        clear_background(color_u8!(31, 31, 31, 0)); // dark gray

        if is_disconnected {
            continue;
        }

        make_egui(&mut mmo_context);

        while let Ok(msg) = cc_rx.try_recv() {
            match msg.msg {
                Cc::PlayerMove {
                    client_request_id,
                    location,
                } => {
                    if client_request_id >= player.request_id {
                        player.prev_location = location;
                        player.curr_location = location;
                    } else {
                        player.prev_location = player.curr_location;
                    }
                }
                Cc::MoveObject { from, to } => {
                    if let Some(val) = game_objects.0.remove(&from) {
                        game_objects.0.insert(to, val);
                    }
                }
                Cc::ChatMsg { from, msg } => {
                    debug!("received message from: {from}. pushing it to the chat.");
                    mmo_context.user_chat.push(ChatMessage::new(from, msg));
                }
                Cc::Pong(ping_id) => ping_monitor.log_ping(&ping_id),
                Cc::Objects(game_obj) => {
                    game_objects = game_obj;
                }
                Cc::Disconnect => {
                    is_disconnected = true;
                }
                Cc::ReconnectOk => {
                    is_disconnected = false;
                }
                Cc::OtherPlayer(op) => match other_players.0.entry(op.username.clone()) {
                    Entry::Occupied(mut entry) => {
                        let player = entry.get_mut();
                        if player.location != op.location || player.direction != op.direction {
                            player.frame = (player.frame + 1) % 3;
                            player.location = op.location;
                            player.direction = op.direction;
                        }
                    }
                    Entry::Vacant(entry) => {
                        entry.insert(OtherPlayer::new(op.username, op.location, op.direction));
                    }
                },
            }
        }

        // Render players
        render_view(&player, &map, &tilesheet);
        player.render(&player_tilesheet);
        player.render_health_bar();
        other_players.render(&player, &player_tilesheet);

        render_objects(
            &player,
            &[&objects_tilesheet_a, &objects_tilesheet_b],
            &game_objects,
        );

        // Handle movement
        fn check_if_player_clicked_on_a_part_of_the_map(player: &Player) {
            if let Some((x, y)) = is_mouse_button_down(MouseButton::Left)
                .then(|| get_mouse_map_tile_position(player.curr_location))
                .flatten()
            {
                debug!("player clicked on: {:?}", (x, y));
            };
        }

        check_if_player_clicked_on_a_part_of_the_map(&player);

        handle_player_movement(&mut player, &other_players);

        // Object movements
        handle_start_move_object(&game_objects, &mut moving_object, &player);
        handle_end_move_object(&mut game_objects, &mut moving_object, &player, &socket);

        // Send player state to server if changed
        send_pos_to_server(&mut player, &socket);

        fps_logger.log_fps();
        ping_monitor.ping_server(&tcp_writer);

        egui_macroquad::draw();

        if is_quit_requested() {
            let ser = bincode::serialize(&TcpClientMsg::Disconnect).unwrap();
            _ = tcp_writer.lock().unwrap().try_write(&ser);
            info!("shutting down client program.");
            std::process::exit(0);
        }

        next_frame().await;
    }
}

fn send_pos_to_server(player: &mut Player, socket: &UdpSocket) {
    if player.curr_location == player.prev_location {
        return;
    }

    let msg = UdpClientMsg::PlayerMove {
        id: player.id,
        client_request_id: {
            player.request_id += 1;
            player.request_id
        },
        location: player.curr_location,
    };
    socket.send_msg_and_log(&msg, None);
}

fn handle_player_movement(player: &mut Player, op: &OtherPlayers) {
    let current_time = get_time();
    let can_move = current_time - player.last_move_timer >= player.speed.into();

    if !can_move {
        return;
    }

    let mut keys_down = get_keys_down();

    if keys_down.len() == 1 {
        handle_single_key_movement(player, op, keys_down.drain().next().unwrap(), current_time);
    } else if keys_down.len() == 2 {
        handle_double_key_movement(player, op, current_time);
    }
}

fn handle_single_key_movement(
    player: &mut Player,
    op: &OtherPlayers,
    key: KeyCode,
    current_time: f64,
) {
    let (x, y) = player.curr_location;
    let (x, y) = (x as i32, y as i32);
    match key {
        KeyCode::Right if Player::can_move((x + 1, y), op) => {
            move_player(player, (1, 0), current_time, BASE_MOVE_DELAY);
        }
        KeyCode::Left if Player::can_move((x - 1, y), op) => {
            move_player(player, (-1, 0), current_time, BASE_MOVE_DELAY);
        }
        KeyCode::Up if Player::can_move((x, y - 1), op) => {
            move_player(player, (0, -1), current_time, BASE_MOVE_DELAY);
        }
        KeyCode::Down if Player::can_move((x, y + 1), op) => {
            move_player(player, (0, 1), current_time, BASE_MOVE_DELAY);
        }
        _ => {}
    }
}

fn handle_double_key_movement(player: &mut Player, op: &OtherPlayers, current_time: f64) {
    let (x, y) = player.curr_location;
    let (x, y) = (x as i32, y as i32);
    if is_key_down(KeyCode::Right)
        && is_key_down(KeyCode::Up)
        && Player::can_move((x + 1, y - 1), op)
    {
        move_player(player, (1, -1), current_time, BASE_MOVE_DELAY * 2.0);
    }
    if is_key_down(KeyCode::Right)
        && is_key_down(KeyCode::Down)
        && Player::can_move((x + 1, y + 1), op)
    {
        move_player(player, (1, 1), current_time, BASE_MOVE_DELAY * 2.0);
    }
    if is_key_down(KeyCode::Left)
        && is_key_down(KeyCode::Up)
        && Player::can_move((x - 1, y - 1), op)
    {
        move_player(player, (-1, -1), current_time, BASE_MOVE_DELAY * 2.0);
    }
    if is_key_down(KeyCode::Left)
        && is_key_down(KeyCode::Down)
        && Player::can_move((x - 1, y + 1), op)
    {
        move_player(player, (-1, 1), current_time, BASE_MOVE_DELAY * 2.0);
    }
}

fn move_player(player: &mut Player, direction: (isize, isize), current_time: f64, speed: f32) {
    player.prev_location = player.curr_location;
    player.curr_location.0 = (player.curr_location.0 as isize + direction.0) as u32;
    player.curr_location.1 = (player.curr_location.1 as isize + direction.1) as u32;
    player.last_move_timer = current_time;
    player.speed = speed;

    debug!("moving player to {:?}", player.curr_location);

    let direction = match direction {
        (1, 0) => Direction::East,
        (-1, 0) => Direction::West,
        (0, -1) => Direction::North,
        (0, 1) => Direction::South,
        (_, 1) => Direction::South,
        (_, -1) => Direction::North,
        (1, _) => Direction::East,
        (-1, _) => Direction::West,
        _ => unreachable!(),
    };

    player.direction = direction;
    player.frame = (player.frame + 1) % 3; // Cycle through frames 0, 1, 2
}

/// Renders the camera around the player
fn render_view(player: &Player, map: &Map, tilesheet: &Tilesheet) {
    for i in 0..CAMERA_HEIGHT {
        for j in 0..CAMERA_WIDTH {
            let x = player.curr_location.0 as i32 - CAMERA_WIDTH as i32 / 2 + j as i32;
            let y = player.curr_location.1 as i32 - CAMERA_HEIGHT as i32 / 2 + i as i32;

            let tile_id = map
                .get_layer(0)
                .and_then(|l| l.as_tile_layer())
                .and_then(|tl| tl.get_tile(x, y))
                .and_then(|t| t.id().into());

            if let Some(t_id) = tile_id {
                tilesheet.render_tile_at(t_id, (j, i));
            } else {
                draw_rectangle(
                    j as f32 * TILE_HEIGHT,
                    i as f32 * TILE_WIDTH,
                    TILE_WIDTH,
                    TILE_HEIGHT,
                    BLACK,
                );
            }
        }
    }
}

fn render_objects(player: &Player, tilesheets: &[&Tilesheet], game_objects: &GameObjects) {
    for i in 0..CAMERA_HEIGHT {
        for j in 0..CAMERA_WIDTH {
            let x = player.curr_location.0 as i32 - CAMERA_WIDTH as i32 / 2 + j as i32;
            let y = player.curr_location.1 as i32 - CAMERA_HEIGHT as i32 / 2 + i as i32;

            if x.is_negative() || y.is_negative() {
                continue;
            }

            let (x, y) = (x as u32, y as u32);

            if !game_objects.0.contains_key(&(x, y)) {
                continue;
            }

            let game_object = &game_objects.0[&(x, y)];

            if let GameObject::Orc { hp, direction, .. } = game_object {
                log::trace!("Orc direction is: {direction:?}",);

                render_entity_name("Orc", (j as f32 * TILE_WIDTH, i as f32 * TILE_HEIGHT));

                let healthbar_pct: f32 = *hp as f32 / ORC_MAX_HP as f32;

                let bar_width = 32.0;
                let bar_height = 4.0;
                let offset_y = -6.0; // move the health bar slightly above the Orc tile

                // background
                draw_rectangle(
                    j as f32 * TILE_WIDTH,
                    i as f32 * TILE_HEIGHT + offset_y,
                    bar_width,
                    bar_height,
                    RED,
                );

                // fill
                draw_rectangle(
                    j as f32 * TILE_WIDTH,
                    i as f32 * TILE_HEIGHT + offset_y,
                    bar_width * healthbar_pct,
                    bar_height,
                    GREEN,
                );

                let tile_id = match direction {
                    Direction::South => 63,
                    Direction::North => 66,
                    Direction::East => 69,
                    Direction::West => 72,
                };
                let tilesheet_number = game_object.tileset_location() - 1;

                tilesheets[tilesheet_number].render_tile_at(tile_id, (j, i));

                continue;
            }

            let tile_id = game_object.id();
            let tilesheet_number = game_object.tileset_location() - 1;

            tilesheets[tilesheet_number].render_tile_at(tile_id, (j, i));
        }
    }
}

fn handle_start_move_object(
    game_objects: &GameObjects,
    moving_object: &mut Option<Location>,
    player: &Player,
) {
    if moving_object.is_some() {
        return;
    }

    if !is_mouse_button_down(MouseButton::Left) {
        return;
    };

    let (x, y) = mouse_position();

    if x < 0. || y < 0. {
        return;
    }

    let (x, y) = ((x / 32.) as usize, (y / 32.) as usize);

    if x >= CAMERA_WIDTH as usize || y >= CAMERA_HEIGHT as usize {
        return;
    }

    let player_x = player.curr_location.0 as i32 - CAMERA_WIDTH as i32 / 2;
    let player_y = player.curr_location.1 as i32 - CAMERA_HEIGHT as i32 / 2;

    let abs_x = player_x + x as i32;
    let abs_y = player_y + y as i32;

    let (x, y) = (abs_x as u32, abs_y as u32);

    if !game_objects.0.contains_key(&(x, y)) {
        return;
    }

    // check if the object is adjacent to the player
    let (obj_x, obj_y) = (x as i32, y as i32);
    let (player_x, player_y) = (player.curr_location.0 as i32, player.curr_location.1 as i32);

    let is_adjacent = (obj_x - player_x).abs() <= 1 && (obj_y - player_y).abs() <= 1;
    if !is_adjacent {
        return;
    }

    *moving_object = Some((x, y));
}

fn handle_end_move_object(
    game_objects: &mut GameObjects,
    moving_object: &mut Option<Location>,
    player: &Player,
    socket: &UdpSocket,
) {
    if moving_object.is_none() || !is_mouse_button_released(MouseButton::Left) {
        return;
    }

    let (x, y) = mouse_position();
    if x < 0. || y < 0. {
        return;
    }

    let (x, y) = ((x / TILE_WIDTH) as usize, (y / TILE_HEIGHT) as usize);
    if x >= CAMERA_WIDTH as usize || y >= CAMERA_HEIGHT as usize {
        return;
    }

    let player_x = player.curr_location.0 as i32 - CAMERA_WIDTH as i32 / 2;
    let player_y = player.curr_location.1 as i32 - CAMERA_HEIGHT as i32 / 2;
    let (abs_x, abs_y) = (player_x + x as i32, player_y + y as i32);
    let (x, y) = (abs_x as u32, abs_y as u32);

    if let Some(moving_obj) = moving_object.take() {
        if let Some(obj) = game_objects.0.remove(&moving_obj) {
            debug!(
                "sending moving object from {:?} to {:?}",
                moving_obj,
                (x, y)
            );

            game_objects.0.insert((x, y), obj);
            let msg = UdpClientMsg::MoveObject {
                id: player.id,
                from: moving_obj,
                to: (x, y),
            };
            socket.send_msg_and_log(&msg, None);
        }
    }
}

async fn request_new_session_from_server(tcp_stream: &mut TcpStream) -> Result<InitPlayer> {
    loop {
        // request user's username for the session
        let mut username = String::new();
        while username.is_empty() {
            println!("Please enter your desired username for the session.");
            let mut reader = BufReader::new(stdin()).lines();
            let Ok(Some(line)) = reader.next_line().await else {
                println!("invalid username. try again.");
                continue;
            };
            let trimmed = line.trim_ascii();
            if trimmed.len() < 4 {
                println!("username too short. try again.");
                continue;
            }
            username = trimmed.to_string();
        }

        // send the username to the server
        let Ok(init_msg) = bincode::serialize(&TcpClientMsg::Init(username.clone())) else {
            println!("failed to serialize message. try again.");
            continue;
        };
        if tcp_stream.write_all(&init_msg).await.is_err() {
            println!("failed to send initiation msg to server. try again.");
            continue;
        }

        // receive response from server
        let mut buf = [0; 1024];

        let Ok(bytes_received) = tcp_stream.read(&mut buf).await else {
            println!("failed to read msg from server. try again.");
            continue;
        };

        // deserialize server msg
        let Ok(sm) = bincode::deserialize::<TcpServerMsg>(&buf[..bytes_received]) else {
            println!("failed to deserialize server msg. try again");
            continue;
        };

        if let TcpServerMsg::InitErr(err) = sm {
            println!("{}", err);
            println!("retrying everything.");
            continue;
        }

        // make sure response is what's expected
        let TcpServerMsg::InitOk(init_player) = sm else {
            println!("expecting an init ok. retrying everything");
            continue;
        };

        return Ok(init_player);
    }
}
