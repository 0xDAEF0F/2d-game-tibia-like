use crate::{Location, MmoTilesheets, constants::*, server::Direction};
use egui_macroquad::macroquad::prelude::*;
use std::collections::{HashMap, VecDeque};
use uuid::Uuid;

#[derive(Debug)]
pub struct Player {
    pub id:              Uuid,
    pub username:        String,
    pub request_id:      u32,
    pub level:           u32,
    pub hp:              u32,
    pub max_hp:          u32,
    pub curr_location:   Location,
    pub prev_location:   Location,
    pub route:           VecDeque<Location>,
    pub last_move_timer: f64,
    pub speed:           f32,
    pub direction:       Direction,
    pub frame:           u32,
}

impl Player {
    /// Renders the player in the middle of the viewport.
    pub fn render(&self, tilesheets: &MmoTilesheets) {
        let x = (CAMERA_WIDTH / 2) as f32 * TILE_WIDTH;
        let y = (CAMERA_HEIGHT / 2) as f32 * TILE_HEIGHT;

        self.render_health_bar();

        render_entity_name(&self.username, (x, y));

        render_player(
            self.direction,
            (CAMERA_WIDTH / 2, CAMERA_HEIGHT / 2),
            tilesheets,
            self.frame,
        );
    }

    /// Renders the player's health bar above the player.
    fn render_health_bar(&self) {
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

        for op in op.0.values() {
            let (px, py) = op.location;
            if (px, py) == (x as u32, y as u32) {
                return false;
            }
        }

        true
    }
}

pub struct OtherPlayers(pub HashMap<String, OtherPlayer>);

impl OtherPlayers {
    pub fn render(&self, player: &Player, tilesheets: &MmoTilesheets) {
        for op in self.0.values() {
            let (x, y) = (op.location.0 as i32, op.location.1 as i32);
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
            let x = x - px + CAMERA_WIDTH as i32 / 2;
            let y = y - py + CAMERA_HEIGHT as i32 / 2;

            render_entity_name(
                &op.username,
                (x as f32 * TILE_WIDTH, y as f32 * TILE_HEIGHT),
            );
            render_player(op.direction, (x as u32, y as u32), tilesheets, op.frame);
        }
    }
}

/// This represents another player from the perspective of
/// the client.
#[derive(Debug)]
pub struct OtherPlayer {
    pub username:  String,
    pub location:  Location,
    pub direction: Direction,
    pub frame:     u32,
}

impl OtherPlayer {
    pub fn new(username: String, location: Location, direction: Direction) -> Self {
        Self {
            username,
            location,
            direction,
            frame: 0,
        }
    }
}

/// Renders only the game master avatar for the time being.
pub fn render_player(
    direction: Direction,
    location: Location,
    tilesheet: &MmoTilesheets,
    frame: u32,
) {
    let tile_to_render = match direction {
        Direction::North => 4 + frame,
        Direction::South => 1 + frame,
        Direction::West => 10 + frame,
        Direction::East => 7 + frame,
    };

    tilesheet.render_tile_at("chars", tile_to_render, location);
}

pub fn render_entity_name(name: &str, screen_location: (f32, f32)) {
    let (x, y) = screen_location;
    let y = y - 10.0; // move the text slightly above the entity 
    draw_text(name, x, y, 20.0 /* font size */, BLACK);
}
