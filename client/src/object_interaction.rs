use crate::Player;
use egui_macroquad::macroquad::prelude::*;
use log::debug;
use shared::{
   GameObjects, Location,
   constants::{CAMERA_HEIGHT, CAMERA_WIDTH, TILE_HEIGHT, TILE_WIDTH},
   network::{sendable::SendableSync, udp::UdpClientMsg},
};
use tokio::net::UdpSocket;

pub fn handle_start_move_object(
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

pub fn handle_end_move_object(
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

   if let Some(moving_obj) = moving_object.take()
      && let Some(obj) = game_objects.0.remove(&moving_obj)
   {
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
