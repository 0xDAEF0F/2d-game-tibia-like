use anyhow::Result;
use bincode;
use game_macroquad_example::{PlayerState, SERVER_ADDR};
use macroquad::Window;
use macroquad::prelude::*;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::UdpSocket;
use tokio::sync::mpsc;
use tokio::sync::mpsc::UnboundedReceiver;

const TILE_WIDTH: f32 = 32.0;
const TILE_HEIGHT: f32 = 32.0;
const BASE_MOVE_DELAY: f32 = 0.2;
const GRID_COLOR: Color = color_u8!(200, 200, 200, 255);

#[derive(Debug)]
struct Player {
    id: SocketAddr,
    curr_location: (usize, usize),
    prev_location: (usize, usize),
    last_move_timer: f64,
    speed: f32,
}

#[tokio::main]
async fn main() -> Result<()> {
    let socket = UdpSocket::bind("0.0.0.0:0").await?;
    socket.connect(SERVER_ADDR).await?;
    let socket = Arc::new(socket);

    println!("client connected to server at: {}", SERVER_ADDR);

    let (tx, rx) = mpsc::unbounded_channel::<(SocketAddr, (usize, usize))>();

    // Spawn async UDP receive task
    let socket_recv = socket.clone();
    tokio::spawn(async move {
        let mut buf = [0; 1024];
        while let Ok(size) = socket_recv.recv(&mut buf).await {
            if let Ok(ps) = bincode::deserialize::<PlayerState>(&buf[..size]) {
                println!("server sends location: {:?}", ps.location);
                _ = tx.send((ps.id, ps.location));
            }
        }
    });

    let conf = Conf {
        window_title: "MMO Game".to_string(),
        ..Default::default()
    };
    Window::from_config(conf, draw(socket, rx));

    Ok(())
}

async fn draw(socket: Arc<UdpSocket>, mut rx: UnboundedReceiver<(SocketAddr, (usize, usize))>) {
    let mut player = Player {
        id: socket.local_addr().unwrap(),
        speed: BASE_MOVE_DELAY,
        curr_location: (0, 0),
        prev_location: (0, 0),
        last_move_timer: 0.0,
    };
    let mut other_players = HashMap::new();
    loop {
        clear_background(color_u8!(31, 31, 31, 0));

        // Render players
        render_player(&player);
        render_other_players(&other_players);

        draw_delimitator_lines();

        // Process server messages from channel
        if let Ok((addr, location)) = rx.try_recv() {
            if addr == player.id {
                player.prev_location = player.curr_location;
                player.curr_location = location;
            } else {
                other_players.insert(addr, location);
            }
        }

        // Handle movement
        handle_player_movement(&mut player);

        // Send player state to server if changed
        send_new_pos_to_server(&player, &socket);

        next_frame().await;
    }
}

fn send_new_pos_to_server(player: &Player, socket: &UdpSocket) {
    if player.curr_location == player.prev_location {
        return;
    }

    let msg = PlayerState {
        id: player.id,
        location: player.curr_location,
    };

    let serialized_message = bincode::serialize(&msg).unwrap();
    _ = socket.try_send(&serialized_message);
}

fn handle_player_movement(player: &mut Player) {
    let current_time = get_time();
    let can_move = current_time - player.last_move_timer >= player.speed.into();

    if !can_move {
        return;
    }

    let mut keys_down = get_keys_down();

    if keys_down.len() == 1 {
        handle_single_key_movement(player, keys_down.drain().next().unwrap(), current_time);
    } else if keys_down.len() == 2 {
        handle_double_key_movement(player, current_time);
    }
}

fn handle_single_key_movement(player: &mut Player, key: KeyCode, current_time: f64) {
    match key {
        KeyCode::Right if player.curr_location.0 < (screen_width() / TILE_WIDTH) as usize - 1 => {
            move_player(player, (1, 0), current_time, BASE_MOVE_DELAY);
        }
        KeyCode::Left if player.curr_location.0 > 0 => {
            move_player(player, (-1, 0), current_time, BASE_MOVE_DELAY);
        }
        KeyCode::Up if player.curr_location.1 > 0 => {
            move_player(player, (0, -1), current_time, BASE_MOVE_DELAY);
        }
        KeyCode::Down if player.curr_location.1 < (screen_height() / TILE_HEIGHT) as usize - 1 => {
            move_player(player, (0, 1), current_time, BASE_MOVE_DELAY);
        }
        _ => {}
    }
}

fn handle_double_key_movement(player: &mut Player, current_time: f64) {
    if is_key_down(KeyCode::Right) && is_key_down(KeyCode::Up) {
        move_player(player, (1, -1), current_time, BASE_MOVE_DELAY * 2.0);
    }
    if is_key_down(KeyCode::Right) && is_key_down(KeyCode::Down) {
        move_player(player, (1, 1), current_time, BASE_MOVE_DELAY * 2.0);
    }
    if is_key_down(KeyCode::Left) && is_key_down(KeyCode::Up) {
        move_player(player, (-1, -1), current_time, BASE_MOVE_DELAY * 2.0);
    }
    if is_key_down(KeyCode::Left) && is_key_down(KeyCode::Down) {
        move_player(player, (-1, 1), current_time, BASE_MOVE_DELAY * 2.0);
    }
}

fn move_player(player: &mut Player, direction: (isize, isize), current_time: f64, speed: f32) {
    player.prev_location = player.curr_location;
    player.curr_location.0 = (player.curr_location.0 as isize + direction.0) as usize;
    player.curr_location.1 = (player.curr_location.1 as isize + direction.1) as usize;
    player.last_move_timer = current_time;
    player.speed = speed;
}

fn render_player(player: &Player) {
    let x = player.curr_location.0 as f32 * TILE_WIDTH;
    let y = player.curr_location.1 as f32 * TILE_HEIGHT;

    draw_rectangle(x, y, TILE_WIDTH, TILE_HEIGHT, color_u8!(255, 0, 0, 255));
}

fn render_other_players(players: &HashMap<SocketAddr, (usize, usize)>) {
    for &(x, y) in players.values() {
        let x = x as f32 * TILE_WIDTH;
        let y = y as f32 * TILE_HEIGHT;

        draw_rectangle(x, y, TILE_WIDTH, TILE_HEIGHT, color_u8!(100, 149, 237, 255));
    }
}

// useful for debugging tiles
fn draw_delimitator_lines() {
    for i in (0..(screen_width() as usize)).step_by(TILE_WIDTH as usize) {
        draw_line(i as f32, 0.0, i as f32, screen_height(), 1.0, GRID_COLOR);
    }
    for j in (0..(screen_height() as usize)).step_by(TILE_HEIGHT as usize) {
        draw_line(0.0, j as f32, screen_width(), j as f32, 1.0, GRID_COLOR);
    }
}
