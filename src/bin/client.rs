use game_macroquad_example::{PlayerState, SERVER_ADDR};
use macroquad::prelude::*;
use renet::{ConnectionConfig, DefaultChannel, RenetClient};
use renet_netcode::{ClientAuthentication, NetcodeClientTransport};
use std::{
    net::UdpSocket,
    time::{Duration, SystemTime},
};

const TILE_WIDTH: f32 = 32.0;
const TILE_HEIGHT: f32 = 32.0;
const BASE_MOVE_DELAY: f32 = 0.2; // Base delay for movement speed
const GRID_COLOR: Color = color_u8!(200, 200, 200, 255); // Light gray grid color

#[derive(Debug, Default)]
struct Player {
    curr_location: (usize, usize),
    prev_location: (usize, usize),
    last_move_timer: f64,
    speed: f32, // Movement speed (lower value = faster movement)
}

#[macroquad::main("MMORPG")]
async fn main() {
    // GAME STATE
    let mut player = Player {
        speed: BASE_MOVE_DELAY,
        last_move_timer: get_time(),
        ..Default::default()
    };

    let mut _last_player_state_confirmation = get_time();
    let mut client = RenetClient::new(ConnectionConfig::default());

    // Setup transport layer using renet_netcode
    let socket = UdpSocket::bind("0.0.0.0:0").unwrap();
    let current_time = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap();
    let authentication = ClientAuthentication::Unsecure {
        server_addr: SERVER_ADDR,
        client_id: 0,
        user_data: None,
        protocol_id: 0,
    };

    let mut transport = NetcodeClientTransport::new(current_time, authentication, socket).unwrap();

    loop {
        clear_background(color_u8!(31, 31, 31, 0));

        render_player(&player);

        draw_delimitator_lines();

        handle_player_movement(&mut player);

        // Receive new messages and update client
        let delta_time = Duration::from_millis(16);
        client.update(delta_time);
        transport.update(delta_time, &mut client).unwrap();

        process_server_msgs(&mut player, &mut client);

        send_new_pos_to_server(&player, &mut client);

        transport.send_packets(&mut client).unwrap();

        next_frame().await
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

// TODO: check if the last position confirmed by the server is the same one
// so you dont spam the server with the same position
fn send_new_pos_to_server(player: &Player, client: &mut RenetClient) {
    if client.is_disconnected() {
        return;
    }

    if player.curr_location == player.prev_location {
        return;
    }

    let msg = PlayerState {
        location: player.curr_location,
    };

    println!(
        "Sending message to server. location: {:?}. timestamp {}",
        msg.location,
        get_time()
    );

    let serialized_message = bincode::serialize(&msg).unwrap();
    client.send_message(DefaultChannel::ReliableOrdered, serialized_message);
}

fn process_server_msgs(player: &mut Player, client: &mut RenetClient) {
    if client.is_disconnected() {
        println!("client disconnected.");
        return;
    }

    while let Some(message) = client.receive_message(DefaultChannel::ReliableOrdered) {
        let ps: PlayerState = bincode::deserialize(&message).unwrap();

        if player.curr_location == ps.location {
            player.prev_location = player.curr_location;
            continue;
        }

        player.prev_location = player.curr_location;
        player.curr_location = ps.location;
    }
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
