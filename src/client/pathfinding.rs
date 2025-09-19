use crate::{
   GameObject, GameObjects, Location,
   client::{OtherPlayers, Player, movement::handle_single_key_movement},
   constants::{CAMERA_HEIGHT, CAMERA_WIDTH, MAP_HEIGHT, MAP_WIDTH, TILE_HEIGHT, TILE_WIDTH},
};
use egui_macroquad::macroquad::prelude::*;
use std::collections::{HashMap, HashSet, VecDeque};

type QuickMap = [[bool; MAP_WIDTH as usize]; MAP_HEIGHT as usize];

pub fn construct_map_from_unwalkable_objects(
   game_objects: &GameObjects,
   other_players: &OtherPlayers,
) -> QuickMap {
   let mut map = [[true; MAP_WIDTH as usize]; MAP_HEIGHT as usize];
   for (location, game_object) in game_objects.0.iter() {
      if let GameObject::Orc { .. } = game_object {
         map[location.1 as usize][location.0 as usize] = false;
      }
   }
   for player in other_players.0.values() {
      map[player.location.1 as usize][player.location.0 as usize] = false;
   }
   map
}

pub fn bfs_find_path(map: &QuickMap, start: Location, end: Location) -> Vec<Location> {
   let mut queue = VecDeque::new();
   let mut visited = HashSet::new();
   let mut came_from: HashMap<Location, Location> = HashMap::new();

   queue.push_back(start);
   visited.insert(start);

   while let Some(current) = queue.pop_front() {
      if current == end {
         // Reconstruct path
         let mut path = vec![current];
         let mut pos = current;
         while pos != start {
            pos = came_from[&pos];
            path.push(pos);
         }
         path.reverse();
         // Remove start location from path
         path.remove(0);
         return path;
      }

      // Check all adjacent tiles
      let possible_moves = [
         (current.0.wrapping_sub(1), current.1), // Left
         (current.0.wrapping_add(1), current.1), // Right
         (current.0, current.1.wrapping_sub(1)), // Up
         (current.0, current.1.wrapping_add(1)), // Down
      ];

      for next in possible_moves {
         // Check if position is within bounds
         if next.0 >= MAP_WIDTH || next.1 >= MAP_HEIGHT {
            continue;
         }

         // Check if position is walkable and not visited
         if map[next.1 as usize][next.0 as usize] && !visited.contains(&next) {
            queue.push_back(next);
            visited.insert(next);
            came_from.insert(next, current);
         }
      }
   }

   vec![]
}

pub fn get_mouse_map_tile_position(player_location: Location) -> Option<(u32, u32)> {
   let (mouse_x, mouse_y) = mouse_position();

   if mouse_x < 0. || mouse_y < 0. {
      return None;
   }

   let (tile_x, tile_y) = (
      (mouse_x / TILE_WIDTH) as usize,
      (mouse_y / TILE_HEIGHT) as usize,
   );

   if tile_x >= CAMERA_WIDTH as usize || tile_y >= CAMERA_HEIGHT as usize {
      return None;
   }

   let player_x = player_location.0 as i32 - CAMERA_WIDTH as i32 / 2;
   let player_y = player_location.1 as i32 - CAMERA_HEIGHT as i32 / 2;

   let abs_x = player_x + tile_x as i32;
   let abs_y = player_y + tile_y as i32;

   if abs_x < 0 || abs_y < 0 {
      return None;
   }

   Some((abs_x as u32, abs_y as u32))
}

pub fn program_route_if_user_clicks_map(
   player: &mut Player,
   game_objects: &GameObjects,
   other_players: &OtherPlayers,
) {
   if !is_mouse_button_pressed(MouseButton::Left) {
      return;
   }

   let Some((x, y)) = get_mouse_map_tile_position(player.curr_location) else {
      return;
   };

   let map = construct_map_from_unwalkable_objects(game_objects, other_players);
   let path = bfs_find_path(&map, player.curr_location, (x, y));

   log::info!("path: {:?}", path);

   if path.is_empty() {
      return;
   }

   player.route = VecDeque::from(path);
}

pub fn handle_route(player: &mut Player, game_objects: &GameObjects, other_players: &OtherPlayers) {
   if player.route.is_empty() {
      return;
   }

   let current_time = get_time();
   let can_move = current_time - player.last_move_timer >= player.speed.into();

   if !can_move {
      return;
   }

   let next_location = player.route.front().unwrap();

   // TODO: there might be other objects on the path that you can't move
   // through
   if let Some(obj) = game_objects.0.get(next_location)
      && obj.is_monster()
   {
      return;
   }

   let key = match (
      next_location.0 as isize - player.curr_location.0 as isize,
      next_location.1 as isize - player.curr_location.1 as isize,
   ) {
      (0, -1) => KeyCode::Up,
      (0, 1) => KeyCode::Down,
      (1, 0) => KeyCode::Right,
      (-1, 0) => KeyCode::Left,
      _ => return,
   };

   handle_single_key_movement(player, other_players, key, get_time());

   player.route.pop_front();
}
