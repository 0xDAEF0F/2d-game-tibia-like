use anyhow::Result;
use bincode;
use game_macroquad_example::Tilesheet;
use game_macroquad_example::{Message, PlayerState, SERVER_UDP_ADDR};
use macroquad::Window;
use macroquad::prelude::*;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tiled::Loader;
use tiled::Map;
use tokio::net::UdpSocket;
use tokio::sync::mpsc::{self, UnboundedReceiver};
use tokio::task::JoinHandle;

const TILE_HEIGHT: f32 = 32.0;
const TILE_WIDTH: f32 = 32.0;

const CAMERA_HEIGHT: u32 = 5;
const CAMERA_WIDTH: u32 = 5;

const MAP_HEIGHT: u32 = 7;
const MAP_WIDTH: u32 = 7;

const BASE_MOVE_DELAY: f32 = 0.2;
const GRID_COLOR: Color = color_u8!(200, 200, 200, 255);

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
    let socket = UdpSocket::bind("0.0.0.0:0").await?;
    let socket = Arc::new(socket);
    socket.connect(SERVER_UDP_ADDR).await?;

    println!("client connected to server at: {}", SERVER_UDP_ADDR);

    let (tx, rx) = mpsc::unbounded_channel::<Message>();

    // Spawn async UDP receive task
    let socket_recv = socket.clone();
    let tx_ = tx.clone();
    tokio::spawn(async move {
        let mut buf = [0; 1024];
        while let Ok(size) = socket_recv.recv(&mut buf).await {
            if let Ok(ps) = bincode::deserialize::<Message>(&buf[..size]) {
                _ = tx_.send(ps);
            }
        }
    });

    let socket_ = socket.clone();
    let _: JoinHandle<Result<()>> = tokio::spawn(async move {
        _ = tokio::signal::ctrl_c().await;

        let serialize = bincode::serialize(&Message::Disconnect)?;
        _ = socket_.try_send(&serialize);

        println!("shutting down client program.");

        std::process::exit(0);
    });

    let conf = Conf {
        window_title: "MMO Game".to_string(),
        ..Default::default()
    };
    Window::from_config(conf, draw(socket, rx));

    Ok(())
}

async fn draw(socket: Arc<UdpSocket>, mut rx: UnboundedReceiver<Message>) {
    let spawn_location = (4, 4);
    let mut player = Player {
        id: socket.local_addr().unwrap(),
        request_id: 0,
        speed: BASE_MOVE_DELAY,
        curr_location: spawn_location,
        prev_location: spawn_location,
        last_move_timer: 0.0,
    };
    let mut other_players = OtherPlayers(HashMap::new());

    let mut loader = Loader::new();
    let map = loader.load_tmx_map("assets/basic-map.tmx").unwrap();
    // println!("Map: {map:?}");
    // println!("{:#?}", map.tilesets()[0].get_tile(0).unwrap());
    let tileset = map.tilesets()[0].clone();
    // println!("{tileset:?}");
    let layer = map.get_layer(0).unwrap();
    let tilelayer = layer.as_tile_layer().unwrap();

    let tilesheet = Tilesheet::from_tileset(tileset);

    let tile = tilelayer.get_tile(0, 0).unwrap();

    // println!("{tile:#?}");

    loop {
        clear_background(color_u8!(31, 31, 31, 0));

        // draw_delimitator_lines();

        // Process server messages from channel
        if let Ok(msg) = rx.try_recv() {
            match msg {
                Message::PlayerState(ps) => {
                    if ps.client_request_id.unwrap() >= player.request_id {
                        player.prev_location = ps.location;
                        player.curr_location = ps.location;
                    } else {
                        player.prev_location = player.curr_location;
                    }
                }
                Message::Disconnect => {
                    println!("sending disconnect to server");
                    let disc_msg = bincode::serialize(&Message::Disconnect).unwrap();
                    _ = socket.try_send(&disc_msg);
                }
                Message::RestOfPlayers(rp) => {
                    let iter = rp.into_iter().map(|p| (p.id, p.location));
                    let new_other_players = HashMap::from_iter(iter);
                    other_players.0 = new_other_players;
                }
            }
        }

        // Render players
        render_view(&player, &map, &tilesheet);
        player.render();
        // println!("{player:?}");
        other_players.render(&player);

        // draw_border_grid();

        // Handle movement
        handle_player_movement(&mut player, &other_players);

        // Send player state to server if changed
        send_new_pos_to_server(&mut player, &socket);

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

    // println!(
    //     "req_id: {} - prev: {:?} - curr: {:?}",
    //     msg.client_request_id, player.prev_location, player.curr_location
    // );

    let serialized_message = bincode::serialize(&Message::PlayerState(ps)).unwrap();
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
    // println!("moving player to {:?}", player.curr_location);
}

// useful for debugging tiles
fn draw_delimitator_lines() {
    let max_x = CAMERA_WIDTH * TILE_WIDTH as u32;
    let max_y = CAMERA_HEIGHT * TILE_HEIGHT as u32;

    for i in (0..max_x).step_by(TILE_WIDTH as usize) {
        draw_line(i as f32, 0.0, i as f32, max_y as f32, 1.0, GRID_COLOR);
    }
    for j in (0..max_y).step_by(TILE_HEIGHT as usize) {
        draw_line(0.0, j as f32, max_x as f32, j as f32, 1.0, GRID_COLOR);
    }
}

fn draw_border_grid() {
    let max_x = CAMERA_WIDTH * TILE_WIDTH as u32;
    let max_y = CAMERA_HEIGHT * TILE_HEIGHT as u32;

    draw_line(0.0, 0.0, max_x as f32, 0.0, 1.0, MAGENTA);
    draw_line(0.0, 0.0, 0.0, max_y as f32, 1.0, MAGENTA);
    draw_line(max_x as f32, 0.0, max_x as f32, max_y as f32, 1.0, MAGENTA);
    draw_line(0.0, max_y as f32, max_x as f32, max_y as f32, 1.0, MAGENTA);
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
                tilesheet.draw_tile_id_at(t_id, (j as u32, i as u32));
            } else {
                draw_rectangle(
                    j as f32 * TILE_HEIGHT,
                    i as f32 * TILE_WIDTH,
                    TILE_WIDTH,
                    TILE_HEIGHT,
                    BLACK,
                );
            }
            // let color = if x < 0 || y < 0 || x >= MAP_WIDTH as i32 || y >= MAP_HEIGHT as i32 {
            //     BLACK
            // } else if (x + y) % 2 == 0 {
            //     WHITE
            // } else {
            //     GRAY
            // };
        }
    }
}
