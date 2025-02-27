use crate::{Location, constants::*, tcp::TcpClientMsg};
use egui_macroquad::macroquad::prelude::*;
use log::trace;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};
use tokio::net::tcp::OwnedWriteHalf;

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

#[derive(Debug, Default)]
pub struct FpsLogger {
    last_log_time: f64,
}

impl FpsLogger {
    pub fn new() -> FpsLogger {
        FpsLogger::default()
    }

    pub fn log_fps(&mut self) {
        let current_time = get_time();

        if current_time - self.last_log_time >= 10. {
            trace!("{}fps", get_fps());
            self.last_log_time = current_time;
        }
    }
}

const PING_INTERVAL: f64 = 5.0;

#[derive(Debug, Default)]
pub struct PingMonitor {
    ping_counter:        u32,
    last_sent_ping_time: f64,
    pings:               HashMap<u32, f64>,
}

impl PingMonitor {
    pub fn new() -> PingMonitor {
        PingMonitor::default()
    }

    pub fn ping_server(&mut self, socket: &Arc<Mutex<OwnedWriteHalf>>) {
        let curr_time = get_time();
        if curr_time - self.last_sent_ping_time >= PING_INTERVAL {
            let ping_id = {
                self.ping_counter += 1;
                self.ping_counter
            };

            let serialized_ping =
                bincode::serialize(&TcpClientMsg::Ping(ping_id)).unwrap();
            _ = socket.lock().unwrap().try_write(&serialized_ping);

            self.pings.insert(ping_id, curr_time);
            self.last_sent_ping_time = curr_time;

            trace!("sending ping request with id: {}", ping_id);
        }
    }

    pub fn log_ping(&mut self, ping_id: &u32) {
        if let Some(ping) = self.pings.remove(ping_id) {
            let now = get_time();

            let latency = (now - ping) * 1_000.0; // ms
            let latency = format!("{:.2}", latency); // formatted

            trace!("ping for req {} = {}ms", ping_id, latency);
        }
    }
}

/// It only returns the mouse position if it is within
/// the macroquad window, else returns `None`. These
/// represent the (x, y) coordinates on the screen.
pub fn get_mouse_position_in_macroquad() -> Option<(f32, f32)> {
    match mouse_position() {
        (x, y) if x >= 0. && y >= 0. => Some((x, y)),
        _ => None,
    }
}

/// If it is outside the camera it will return `None`
pub fn get_mouse_camera_tile_position() -> Option<(u32, u32)> {
    get_mouse_position_in_macroquad().and_then(|(x, y)| {
        let (x, y) = (x / TILE_WIDTH, y / TILE_WIDTH); // `TILE_HEIGHT` == `TILE_WIDTH`
        let (x, y) = (x as u32, y as u32);
        match (x, y) {
            (x, y) if x < CAMERA_WIDTH && y < CAMERA_HEIGHT => Some((x, y)),
            _ => None,
        }
    })
}

pub fn get_mouse_map_tile_position(player_location: Location) -> Option<(u32, u32)> {
    let (px, py) = (player_location.0 as i32, player_location.1 as i32);
    get_mouse_camera_tile_position().and_then(|(x, y)| {
        let origin_x = px - (CAMERA_WIDTH as i32 / 2);
        let origin_y = py - (CAMERA_HEIGHT as i32 / 2);
        match (origin_x + x as i32, origin_y + y as i32) {
            (x, y) if x.is_negative() || y.is_negative() => None,
            (x, y)
                if x >= MAP_WIDTH.try_into().ok()?
                    || y >= MAP_HEIGHT.try_into().ok()? =>
            {
                None
            }
            (x, y) => Some((x as u32, y as u32)),
        }
    })
}
