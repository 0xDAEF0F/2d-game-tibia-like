use crate::Location;
use crate::Tilesheet;
use crate::constants::*;
use egui_macroquad::macroquad::prelude::*;
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug)]
pub struct Player {
    pub id: Uuid,
    pub username: String,
    pub request_id: u32,
    pub level: u32,
    pub hp: u32,
    pub max_hp: u32,
    pub curr_location: Location,
    pub prev_location: Location,
    pub last_move_timer: f64,
    pub speed: f32,
}

impl Player {
    /// Renders the player in the middle of the viewport.
    pub fn render(&self, tilesheet: &Tilesheet) {
        // Note: this is in screen coordinates.
        let x = (CAMERA_WIDTH / 2) as f32 * TILE_WIDTH;
        let y = (CAMERA_HEIGHT / 2) as f32 * TILE_HEIGHT;

        draw_text(&self.username, x, y - 10.0, 20.0, BLACK);

        tilesheet.render_tile_at(2, (CAMERA_WIDTH / 2, CAMERA_HEIGHT / 2));
    }

    /// Renders the player's health bar above the player.
    pub fn render_health_bar(&self) {
        let healthbar_pct: f32 = self.hp as f32 / self.max_hp as f32;

        let bar_width = 32.0;
        let bar_height = 4.0;
        let offset_y = -6.0; // move the health bar slightly above the player tile

        // background
        draw_rectangle(
            (CAMERA_WIDTH / 2) as f32 * TILE_WIDTH,
            (CAMERA_HEIGHT / 2) as f32 * TILE_HEIGHT + offset_y,
            bar_width,
            bar_height,
            RED,
        );

        // fill
        draw_rectangle(
            (CAMERA_WIDTH / 2) as f32 * TILE_WIDTH,
            (CAMERA_HEIGHT / 2) as f32 * TILE_HEIGHT + offset_y,
            bar_width * healthbar_pct,
            bar_height,
            GREEN,
        );
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
