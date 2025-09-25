use crate::{MmoTilesheets, Player, player::render_entity_name};
use egui_macroquad::macroquad::prelude::*;
use shared::{
   Direction, GameObject, GameObjects,
   constants::{CAMERA_HEIGHT, CAMERA_WIDTH, TILE_HEIGHT, TILE_WIDTH},
   game_objects::ORC_MAX_HP,
};
use thin_logger::log::trace;
use tiled::Map;

pub fn render_view(player: &Player, map: &Map, tilesheets: &MmoTilesheets) {
   for i in 0..CAMERA_HEIGHT {
      for j in 0..CAMERA_WIDTH {
         let x = player.curr_location.0 as i32 - CAMERA_WIDTH as i32 / 2 + j as i32;
         let y = player.curr_location.1 as i32 - CAMERA_HEIGHT as i32 / 2 + i as i32;

         let mut tile_drawn = false;

         // Iterate through all layers to find the correct z-level
         for (group_idx, layer) in map.layers().enumerate() {
            // z_level 0 = base group (first group), z_level 1 = top group (second group)
            let tile = match layer.layer_type() {
               tiled::LayerType::Group(group_layer) if group_idx == player.z_level as usize => {
                  // Look for tile layers inside the current z-level group
                  group_layer.layers().find_map(|l| match l.layer_type() {
                     tiled::LayerType::Tiles(tl) => tl.get_tile(x, y),
                     _ => None,
                  })
               }
               _ => None,
            };

            if let Some(t) = tile
               && let Some(t_id) = t.id().into()
            {
               tilesheets.render_tile_at("grass-tileset", t_id, (j, i, 0));
               tile_drawn = true;
               break;
            }
         }

         if !tile_drawn {
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

pub fn render_objects(player: &Player, tilesheets: &MmoTilesheets, game_objects: &GameObjects) {
   for i in 0..CAMERA_HEIGHT {
      for j in 0..CAMERA_WIDTH {
         let x = player.curr_location.0 as i32 - CAMERA_WIDTH as i32 / 2 + j as i32;
         let y = player.curr_location.1 as i32 - CAMERA_HEIGHT as i32 / 2 + i as i32;

         if x.is_negative() || y.is_negative() {
            continue;
         }

         let (x, y) = (x as u32, y as u32);

         // Check if object exists at player's current z_level
         let object_location = (x, y, player.z_level);
         if !game_objects.0.contains_key(&object_location) {
            continue;
         }

         let game_object = &game_objects.0[&object_location];

         if let GameObject::Orc { hp, direction, .. } = game_object {
            trace!("Orc direction is: {direction:?}",);

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

            tilesheets.render_tile_at("tibia-sprites", tile_id, (j, i, 0));
            continue;
         }

         tilesheets.render_tile_at("props-tileset", game_object.id(), (j, i, 0));
      }
   }
}
