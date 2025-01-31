use log::debug;
use macroquad::prelude::*;

const TILE_WIDTH: f32 = 32.0;
const TILE_HEIGHT: f32 = 32.0;

const CAMERA_WIDTH: u32 = 10;
const CAMERA_HEIGHT: u32 = 10;

pub fn draw_delimitator_lines() {
    let max_x = CAMERA_WIDTH * TILE_WIDTH as u32;
    let max_y = CAMERA_HEIGHT * TILE_HEIGHT as u32;

    for i in (0..max_x).step_by(TILE_WIDTH as usize) {
        draw_line(i as f32, 0.0, i as f32, max_y as f32, 1.0, LIGHTGRAY);
    }
    for j in (0..max_y).step_by(TILE_HEIGHT as usize) {
        draw_line(0.0, j as f32, max_x as f32, j as f32, 1.0, LIGHTGRAY);
    }
}

pub fn draw_border_grid() {
    let max_x = CAMERA_WIDTH * TILE_WIDTH as u32;
    let max_y = CAMERA_HEIGHT * TILE_HEIGHT as u32;

    draw_line(0.0, 0.0, max_x as f32, 0.0, 1.0, MAGENTA);
    draw_line(0.0, 0.0, 0.0, max_y as f32, 1.0, MAGENTA);
    draw_line(max_x as f32, 0.0, max_x as f32, max_y as f32, 1.0, MAGENTA);
    draw_line(0.0, max_y as f32, max_x as f32, max_y as f32, 1.0, MAGENTA);
}

pub struct FpsLogger {
    last_log_time: f64,
}

impl FpsLogger {
    pub fn new() -> FpsLogger {
        FpsLogger {
            last_log_time: f64::default(),
        }
    }

    pub fn log_fps(&mut self) {
        let current_time = get_time();

        if current_time - self.last_log_time >= 10. {
            debug!("{}fps", get_fps());
            self.last_log_time = current_time;
        }
    }
}
