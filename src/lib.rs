mod client;
mod game_objects;
mod logger;
pub mod server;
mod server_state;
mod tilesheet;
mod utils;

pub use game_objects::*;
pub use logger::*;
pub use server_state::*;
pub use tilesheet::*;
pub use utils::*;

use serde::{Deserialize, Serialize};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};

pub const TILE_WIDTH: f32 = 32.0;
pub const TILE_HEIGHT: f32 = 32.0;

pub const SERVER_UDP_ADDR: SocketAddr =
    SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 5000);
pub const SERVER_TCP_ADDR: &str = "127.0.0.1:8080";

pub type Location = (usize, usize); // (x, y) coordinates

// Client -> Server
#[derive(Debug, Serialize, Deserialize)]
pub enum ClientMsg<'a> {
    PlayerState(PlayerState),
    MoveObject {
        from: (usize, usize),
        to: (usize, usize),
    },
    Disconnect,
    Ping(u32),
    ChatMsg(&'a str),
    Init(String),
}

// Server -> Client
#[derive(Debug, Serialize, Deserialize)]
pub enum ServerMsg {
    PlayerState(PlayerState),
    RestOfPlayers(Vec<PlayerState>),
    Objects(GameObjects),
    Pong(u32),
    ChatMsg(String),
    InitOk(usize, Location), // id, location
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PlayerState {
    pub id: SocketAddr,                 // TODO: use another identifier
    pub client_request_id: Option<u64>, // TODO: use another identifier
    pub location: (usize, usize),       // (x, y)
}
