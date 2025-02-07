pub mod client;
pub mod constants;
mod game_objects;
mod logger;
pub mod server;
mod tilesheet;
mod utils;

pub use game_objects::*;
pub use logger::*;
pub use tilesheet::*;
pub use utils::*;

use serde::{Deserialize, Serialize};
use std::net::SocketAddr;

pub type Location = (usize, usize); // (x, y) coordinates

// Client -> Server
#[derive(Debug, Serialize, Deserialize)]
pub enum ClientMsg {
    PlayerState(PlayerState),
    MoveObject { from: Location, to: Location },
    Disconnect,
    Ping(u32),
    ChatMsg(String),
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
