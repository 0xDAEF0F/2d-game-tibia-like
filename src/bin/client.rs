use anyhow::Result;
use bincode;
use env_logger::Env;
use game_macroquad_example::*;
use log::{debug, info};
use macroquad::Window;
use macroquad::prelude::*;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tiled::{Loader, Map};
use tokio::net::UdpSocket;
use tokio::sync::mpsc::{self, UnboundedReceiver};
use tokio::task::JoinHandle;

const CAMERA_WIDTH: u32 = 10;
const CAMERA_HEIGHT: u32 = 10;

const MAP_WIDTH: u32 = 30;
const MAP_HEIGHT: u32 = 20;

const BASE_MOVE_DELAY: f32 = 0.2;

#[derive(Debug)]
struct Player {
    id: SocketAddr,
    request_id: u64,
    curr_location: (usize, usize),
    prev_location: (usize, usize),
    last_move_timer: f64,
    speed: f32,
}

impl Player {
    /// Renders the player in the middle of the viewport.
    pub fn render(&self) {
        let x = (CAMERA_WIDTH / 2) as f32 * TILE_WIDTH;
        let y = (CAMERA_HEIGHT / 2) as f32 * TILE_HEIGHT;
        draw_rectangle(x, y, TILE_WIDTH, TILE_HEIGHT, RED);
    }

    pub fn can_move((x, y): (i32, i32), op: &OtherPlayers) -> bool {
        if x.is_negative() || y.is_negative() {
            return false;
        }

        if x > (MAP_WIDTH - 1) as i32 || y > (MAP_HEIGHT - 1) as i32 {
            return false;
        }

        for &(px, py) in op.0.values() {
            if (px, py) == (x as usize, y as usize) {
                return false;
            }
        }

        return true;
    }
}

struct OtherPlayers(HashMap<SocketAddr, (usize, usize)>);

impl OtherPlayers {
    pub fn render(&self, player: &Player) {
        for &(x, y) in self.0.values() {
            let (x, y) = (x as i32, y as i32);
            let (px, py) = (player.curr_location.0 as i32, player.curr_location.1 as i32);

            let relative_offset_x = (CAMERA_WIDTH / 2) as i32;
            let relative_offset_y = (CAMERA_HEIGHT / 2) as i32;

            // is the `other_player` outside the viewport?
            if x < px - relative_offset_x
                || x > px + relative_offset_x
                || y < py - relative_offset_y
                || y > py + relative_offset_y
            {
                continue;
            }

            // determine where to render relative to the player
            let x = (x as i32 - px as i32 + CAMERA_WIDTH as i32 / 2) as f32 * TILE_WIDTH;
            let y = (y as i32 - py as i32 + CAMERA_HEIGHT as i32 / 2) as f32 * TILE_HEIGHT;

            draw_rectangle(x, y, TILE_WIDTH, TILE_HEIGHT, MAGENTA);
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let env = Env::default().default_filter_or("debug");
    env_logger::init_from_env(env);

    let socket = UdpSocket::bind("0.0.0.0:0").await?;
    let socket = Arc::new(socket);
    socket.connect(SERVER_UDP_ADDR).await?;

    info!("client connected to server at: {}", SERVER_UDP_ADDR);

    let (tx, rx) = mpsc::unbounded_channel::<ServerMsg>();

    // Spawn async UDP receive task
    let socket_recv = socket.clone();
    let tx_ = tx.clone();
    tokio::spawn(async move {
        let mut buf = [0; 1024];
        while let Ok(size) = socket_recv.recv(&mut buf).await {
            if let Ok(ps) = bincode::deserialize::<ServerMsg>(&buf[..size]) {
                _ = tx_.send(ps);
            }
        }
    });

    let socket_ = socket.clone();
    let _: JoinHandle<Result<()>> = tokio::spawn(async move {
        _ = tokio::signal::ctrl_c().await;

        let serialize = bincode::serialize(&ClientMsg::Disconnect)?;
        _ = socket_.try_send(&serialize);

        info!("shutting down client program.");

        std::process::exit(0);
    });

    let conf = Conf {
        window_title: "MMO Game".to_string(),
        ..Default::default()
    };
    Window::from_config(conf, draw(socket, rx));

    Ok(())
}

async fn draw(socket: Arc<UdpSocket>, mut rx: UnboundedReceiver<ServerMsg>) {
    let spawn_location = ((MAP_WIDTH / 2) as usize, (MAP_HEIGHT / 2) as usize);
    let mut player = Player {
        id: socket.local_addr().unwrap(),
        request_id: 0,
        speed: BASE_MOVE_DELAY,
        curr_location: spawn_location,
        prev_location: spawn_location,
        last_move_timer: 0.0,
    };
    let mut other_players = OtherPlayers(HashMap::new());

    let map = {
        let mut loader = Loader::new();
        loader.load_tmx_map("assets/basic-map.tmx").unwrap()
    };

    let tilesheet = Tilesheet::from_tileset(map.tilesets()[0].clone());
    let objects_tilesheet = Tilesheet::from_tileset(map.tilesets()[1].clone());

    let mut game_objects = create_game_objects();
    let mut moving_object: Option<(usize, usize)> = None;

    let mut fps_logger = FpsLogger::new();

    let mut ping_counter: u32 = 0;
    let mut last_sent_ping_request = get_time();
    let mut pings = HashMap::new();

    loop {
        let dark_gray = color_u8!(31, 31, 31, 0);
        clear_background(dark_gray);

        while let Ok(msg) = rx.try_recv() {
            match msg {
                ServerMsg::PlayerState(ps) => {
                    if ps.client_request_id.unwrap() >= player.request_id {
                        player.prev_location = ps.location;
                        player.curr_location = ps.location;
                    } else {
                        player.prev_location = player.curr_location;
                    }
                }
                ServerMsg::Pong(ping_id) => {
                    if let Some(ping) = pings.remove(&ping_id) {
                        let latency = (get_time() - ping) * 1_000.0;
                        let latency = format!("{:.2}", latency);
                        debug!("ping_id: {} = {}ms", ping_id, latency);
                    }
                }
                ServerMsg::Objects(o) => {
                    if !game_objects.eq(&o) {
                        debug!("updating game objects");
                        game_objects = o.clone();
                    }
                }
                ServerMsg::RestOfPlayers(rp) => {
                    let iter = rp.into_iter().map(|p| (p.id, p.location));
                    let new_other_players = HashMap::from_iter(iter);
                    other_players.0 = new_other_players;
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
        send_new_pos_to_server(&mut player, &socket);

        fps_logger.log_fps();

        if get_time() - last_sent_ping_request >= 4.0 {
            let ping_id = {
                ping_counter += 1;
                ping_counter
            };
            let ping = get_time();
            let serialized_ping = bincode::serialize(&ClientMsg::Ping(ping_id)).unwrap();
            _ = socket.try_send(&serialized_ping);
            debug!("sending ping request with id: {}", ping_id);
            pings.insert(ping_id, ping);
            last_sent_ping_request = get_time();
        }

        next_frame().await;
    }
}

fn send_new_pos_to_server(player: &mut Player, socket: &UdpSocket) {
    if player.curr_location == player.prev_location {
        return;
    }

    let ps = PlayerState {
        id: player.id,
        client_request_id: {
            player.request_id += 1;
            Some(player.request_id)
        },
        location: player.curr_location,
    };

    let serialized_message = bincode::serialize(&ClientMsg::PlayerState(ps)).unwrap();
    _ = socket.try_send(&serialized_message);
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
    player.curr_location.0 = (player.curr_location.0 as isize + direction.0) as usize;
    player.curr_location.1 = (player.curr_location.1 as isize + direction.1) as usize;
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
                .map(|t| t.id());

            if let Some(t_id) = tile_id {
                tilesheet.render_tile_at(t_id, (j as u32, i as u32));
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

            let (x, y) = (x as usize, y as usize);

            if !game_objects.0.contains_key(&(x, y)) {
                continue;
            }

            let go = &game_objects.0[&(x, y)];
            let tile_id = go.into();

            tilesheet.render_tile_at(tile_id, (j as u32, i as u32));
        }
    }
}

fn handle_start_move_object(
    game_objects: &GameObjects,
    moving_object: &mut Option<(usize, usize)>,
    player: &Player,
) {
    if moving_object.is_some() {
        return;
    }

    if !is_mouse_button_down(MouseButton::Left) {
        return;
    };

    let (x, y) = mouse_position();

    assert!(x >= 0. && y >= 0.);

    let (x, y) = ((x / 32.) as usize, (y / 32.) as usize);

    if x >= CAMERA_WIDTH as usize || y >= CAMERA_HEIGHT as usize {
        return;
    }

    let player_x = player.curr_location.0 as i32 - CAMERA_WIDTH as i32 / 2;
    let player_y = player.curr_location.1 as i32 - CAMERA_HEIGHT as i32 / 2;

    let abs_x = player_x + x as i32;
    let abs_y = player_y + y as i32;

    let (x, y) = (abs_x as usize, abs_y as usize);

    if !game_objects.0.contains_key(&(x, y)) {
        return;
    }

    *moving_object = Some((x, y));
}

fn handle_end_move_object(
    game_objects: &mut GameObjects,
    moving_object: &mut Option<(usize, usize)>,
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
    let (x, y) = (abs_x as usize, abs_y as usize);

    if let Some(moving_obj) = moving_object.take() {
        if let Some(obj) = game_objects.0.remove(&moving_obj) {
            debug!(
                "sending moving object from {:?} to {:?}",
                moving_obj,
                (x, y)
            );

            game_objects.0.insert((x, y), obj);
            let msg = bincode::serialize(&ClientMsg::MoveObject {
                from: moving_obj,
                to: (x, y),
            })
            .unwrap();
            _ = socket.try_send(&msg);
        }
    }
}
