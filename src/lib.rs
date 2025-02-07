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
use uuid::Uuid;

pub type Location = (u32, u32); // (x, y) coordinates

// Client -> Server
#[derive(Debug, Serialize, Deserialize)]
pub enum ClientMsg {
    PlayerState {
        id: Uuid,
        location: Location,
        client_request_id: u32,
    },
    MoveObject {
        from: Location,
        to: Location,
    },
    Disconnect,
    Ping(u32),
    ChatMsg(String),
    Init(String),
}

// Server -> Client
#[derive(Debug, Serialize, Deserialize)]
pub enum ServerMsg {
    PlayerState {
        location: Location,
        client_request_id: u32,
    },
    RestOfPlayers(Vec<OtherPlayer>),
    Objects(GameObjects),
    Pong(u32),
    ChatMsg(String),
    InitOk(Uuid, Location),
    InitErr(String),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OtherPlayer {
    pub username: String,
    pub location: Location,
}
