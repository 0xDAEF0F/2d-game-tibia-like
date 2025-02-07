use crate::Location;
use crate::client::constants::*;
use crate::constants::*;
use egui_macroquad::macroquad::prelude::*;
use std::{collections::HashMap, net::SocketAddr};
use uuid::Uuid;

#[derive(Debug)]
pub struct Player {
    pub id: Uuid,
    pub username: String,
    pub request_id: u32,
    pub curr_location: Location,
    pub prev_location: Location,
    pub last_move_timer: f64,
    pub speed: f32,
}

impl Player {
    /// Renders the player in the middle of the viewport.
    pub fn render(&self) {
        let x = (CAMERA_WIDTH / 2) as f32 * TILE_WIDTH;
        let y = (CAMERA_HEIGHT / 2) as f32 * TILE_HEIGHT;
        draw_rectangle(x, y, TILE_WIDTH, TILE_HEIGHT, RED);

        // TODO: refactor this and need to center the text correctly above the player
        // draw its username as text right above the player
        // let text_dimensions = measure_text(&self.username, None, 20, 1.0);
        draw_text(&self.username, x, y - 10.0, 20.0, BLACK);
    }

    pub fn can_move((x, y): (i32, i32), op: &OtherPlayers) -> bool {
        if x.is_negative() || y.is_negative() {
            return false;
        }

        if x > (MAP_WIDTH - 1) as i32 || y > (MAP_HEIGHT - 1) as i32 {
            return false;
        }

        for &(px, py) in op.0.values() {
            if (px, py) == (x as u32, y as u32) {
                return false;
            }
        }

        true
    }
}

pub struct OtherPlayers(pub HashMap<String, Location>);

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
            let x = (x - px + CAMERA_WIDTH as i32 / 2) as f32 * TILE_WIDTH;
            let y = (y - py + CAMERA_HEIGHT as i32 / 2) as f32 * TILE_HEIGHT;

            draw_rectangle(x, y, TILE_WIDTH, TILE_HEIGHT, MAGENTA);
        }
    }
}
