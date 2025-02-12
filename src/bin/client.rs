use anyhow::Result;
use egui_macroquad::macroquad;
use log::{debug, info};
use macroquad::Window;
use macroquad::prelude::*;
use my_mmo::client::Cc;
use my_mmo::client::ChatMessage;
use my_mmo::client::ClientChannel;
use my_mmo::client::tasks::tcp_reader_task;
use my_mmo::client::tasks::udp_recv_task;
use my_mmo::client::{MmoContext, OtherPlayers, Player, make_egui};
use my_mmo::constants::*;
use my_mmo::*;
use std::collections::HashMap;
use std::sync::Arc;
use tiled::{Loader, Map};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader, stdin};
use tokio::net::TcpStream;
use tokio::net::{TcpSocket, UdpSocket, tcp::OwnedWriteHalf};
use tokio::sync::mpsc::{self, UnboundedReceiver};
use uuid::Uuid;

#[tokio::main]
async fn main() -> Result<()> {
    MmoLogger::init("debug");

    let socket = UdpSocket::bind("0.0.0.0:0").await?;
    let socket = Arc::new(socket);
    socket.connect(SERVER_UDP_ADDR).await?;

    let tcp_socket = TcpSocket::new_v4()?;
    let mut stream = tcp_socket.connect(SERVER_TCP_ADDR.parse()?).await?;

    let (username, user_id, spawn_location) = request_new_session_from_server(&mut stream).await?;

    println!("username: {} was accepted by the server.", username);

    let (tcp_read, tcp_write) = stream.into_split();

    info!("client connected to server at: {}", SERVER_TCP_ADDR);

    let (cc_tx, cc_rx) = mpsc::unbounded_channel::<ClientChannel>();

    // Spawn async UDP receive task
    let join_handle1 = udp_recv_task(socket.clone(), cc_tx.clone(), user_id);

    // TCP reader
    let join_handle2 = tcp_reader_task(tcp_read, cc_tx.clone(), user_id);

    tokio::spawn(async move {
        let Ok((t1, t2)) = tokio::try_join!(join_handle1, join_handle2) else {
            error!("failed to join UDP receive and TCP reader tasks.");
            std::process::exit(1);
        };

        if let Err(e) = t1 {
            error!("UDP receive task failed: {e}");
            std::process::exit(1);
        }

        if let Err(e) = t2 {
            error!("TCP reader task failed: {e}");
            std::process::exit(1);
        }

        std::process::exit(0);
    });

    // Macroquad configuration and window
    let conf = Conf {
        window_title: "MMO Game".to_string(),
        high_dpi: true,
        ..Default::default()
    };
    let player = Player {
        id: user_id,
        username,
        request_id: 0,
        speed: BASE_MOVE_DELAY,
        curr_location: spawn_location,
        prev_location: spawn_location,
        last_move_timer: 0.0,
    };
    Window::from_config(conf, draw(socket, cc_rx, tcp_write, player));

    Ok(())
}

async fn draw(
    socket: Arc<UdpSocket>,
    mut rx: UnboundedReceiver<ClientChannel>,
    tcp_writer: OwnedWriteHalf,
    mut player: Player,
) {
    prevent_quit();

    let mut other_players = OtherPlayers(HashMap::new());

    let map = {
        let mut loader = Loader::new();
        loader.load_tmx_map("assets/basic-map.tmx").unwrap()
    };

    let tilesheet = Tilesheet::from_tileset(map.tilesets()[0].clone());
    let objects_tilesheet = Tilesheet::from_tileset(map.tilesets()[1].clone());

    let mut game_objects = GameObjects::new();
    let mut moving_object: Option<Location> = None;

    let mut fps_logger = FpsLogger::new();
    let mut ping_monitor = PingMonitor::new();

    let mut mmo_context = MmoContext {
        username: player.username.clone(),
        user_text: "".to_string(),
        user_chat: vec![],
        server_tcp_write_stream: &tcp_writer,
    };

    loop {
        clear_background(color_u8!(31, 31, 31, 0)); // dark gray

        make_egui(&mut mmo_context);

        while let Ok(msg) = rx.try_recv() {
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
                Cc::Disconnect => std::process::exit(0),
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
                Cc::RestOfPlayers(op) => {
                    let iter = op.into_iter().map(|p| (p.username, p.location));
                    other_players.0.clear();
                    other_players.0.extend(iter);
                }
                Cc::Objects(game_obj) => {
                    game_objects = game_obj;
                }
            }
        }

        // Render players
        render_view(&player, &map, &tilesheet);
        player.render();
        other_players.render(&player);

        render_objects(&player, &objects_tilesheet, &game_objects);

        // Handle movement
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
            _ = tcp_writer.try_write(&ser);
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
    let ser = bincode::serialize(&msg).unwrap();

    if let Err(e) = socket.try_send(&ser) {
        error!("failed to send UDP message to server: {e}");
    };
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

fn render_objects(player: &Player, tilesheet: &Tilesheet, game_objects: &GameObjects) {
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

            let go = &game_objects.0[&(x, y)];
            let tile_id = go.into();

            tilesheet.render_tile_at(tile_id, (j, i));
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
            let msg = bincode::serialize(&TcpClientMsg::MoveObject {
                from: moving_obj,
                to: (x, y),
            })
            .unwrap();
            _ = socket.try_send(&msg);
        }
    }
}

async fn request_new_session_from_server(
    tcp_stream: &mut TcpStream,
) -> Result<(String, Uuid, Location)> {
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
        let TcpServerMsg::InitOk(id, location) = sm else {
            println!("expecting an init ok. retrying everything");
            continue;
        };

        return Ok((username, id, location));
    }
}
