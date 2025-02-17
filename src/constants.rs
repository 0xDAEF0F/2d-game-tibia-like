// Global
pub const TILE_WIDTH: f32 = 32.0;
pub const TILE_HEIGHT: f32 = 32.0;

// Server
pub const SERVER_TICK_RATE: u64 = 16; // how often the server loops. ms.

pub const SERVER_UDP_ADDR: &str = "127.0.0.1:5000";
pub const SERVER_TCP_ADDR: &str = "127.0.0.1:8080";

// Client
pub const CAMERA_WIDTH: u32 = 19;
pub const CAMERA_HEIGHT: u32 = 15;

pub const MAP_WIDTH: u32 = 30;
pub const MAP_HEIGHT: u32 = 20;

pub const BASE_MOVE_DELAY: f32 = 0.2; // expressed in seconds (1 tile / 0.2 secs)

pub const MAX_CONNECTION_RETRIES: u8 = 5;
