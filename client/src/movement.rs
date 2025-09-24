use crate::{OtherPlayers, Player};
use egui_macroquad::macroquad::prelude::*;
use log::debug;
use shared::{Direction, constants::BASE_MOVE_DELAY};

pub fn send_pos_to_server(player: &mut Player, socket: &tokio::net::UdpSocket) {
   use shared::network::{sendable::SendableSync, udp::UdpClientMsg};

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

pub fn handle_player_movement(player: &mut Player, op: &OtherPlayers) {
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

pub fn handle_single_key_movement(
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

pub fn handle_double_key_movement(player: &mut Player, op: &OtherPlayers, current_time: f64) {
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
   if is_key_down(KeyCode::Left) && is_key_down(KeyCode::Up) && Player::can_move((x - 1, y - 1), op)
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

pub fn move_player(player: &mut Player, direction: (isize, isize), current_time: f64, speed: f32) {
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
