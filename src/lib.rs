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
use uuid::Uuid;

pub type Location = (u32, u32); // (x, y) coordinates

// Client -> Server
#[derive(Debug, Serialize, Deserialize)]
pub enum TcpClientMsg {
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

// Client -> Server
#[derive(Debug, Serialize, Deserialize)]
pub enum UdpClientMsg {
    PlayerMove {
        id: Uuid,
        client_request_id: u32,
        location: Location,
    },
    Ping {
        id: Uuid,
        client_request_id: u32,
    },
}

impl UdpClientMsg {
    pub fn get_player_id(&self) -> Uuid {
        let id = match self {
            UdpClientMsg::Ping { id, .. } => id,
            UdpClientMsg::PlayerMove { id, .. } => id,
        };
        *id
    }
}

// Server -> Client
#[derive(Debug, Serialize, Deserialize)]
pub enum UdpServerMsg {
    PlayerMove {
        location: Location,
        client_request_id: u32,
    },
    RestOfPlayers(Vec<OtherPlayer>),
    Objects(GameObjects),
    Pong(u32),
}

#[derive(Debug, Serialize, Deserialize)]
pub enum TcpServerMsg {
    Pong(u32),
    ChatMsg { username: String, msg: String },
    InitOk(Uuid, Location),
    InitErr(String),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OtherPlayer {
    pub username: String,
    pub location: Location,
}
