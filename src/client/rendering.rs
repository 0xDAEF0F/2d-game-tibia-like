use crate::{
   GameObject, GameObjects, MmoTilesheets,
   client::{Player, render_entity_name},
   constants::{CAMERA_HEIGHT, CAMERA_WIDTH, TILE_HEIGHT, TILE_WIDTH},
   game_objects::ORC_MAX_HP,
   server::Direction,
};
use egui_macroquad::macroquad::prelude::*;
use tiled::Map;

pub fn render_view(player: &Player, map: &Map, tilesheets: &MmoTilesheets) {
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
            tilesheets.render_tile_at("grass-tileset", t_id, (j, i));
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

pub fn render_objects(player: &Player, tilesheets: &MmoTilesheets, game_objects: &GameObjects) {
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

            tilesheets.render_tile_at("tibia-sprites", tile_id, (j, i));
            continue;
         }

         tilesheets.render_tile_at("props-tileset", game_object.id(), (j, i));
      }
   }
}
