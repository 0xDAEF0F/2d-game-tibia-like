pub mod client;
pub mod constants;
mod game_objects;
mod logger;
pub mod server;
mod tilesheet;
mod utils;

pub use game_objects::*;
pub use logger::*;
use server::Direction;
pub use tilesheet::*;
use tokio::net::UdpSocket;
pub use utils::*;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use uuid::Uuid;

pub type Location = (u32, u32); // (x, y) coordinates

pub trait SendableSync {
    fn send_msg<T: Serialize>(&self, msg: &T, to: SocketAddr) -> Result<usize>;
}

impl SendableSync for UdpSocket {
    fn send_msg<T: Serialize>(&self, msg: &T, to: SocketAddr) -> Result<usize> {
        let buf = bincode::serialize(msg)?;
        Ok(self.try_send_to(&buf, to)?)
    }
}

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
    Reconnect(Uuid),
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
        match self {
            UdpClientMsg::Ping { id, .. } => *id,
            UdpClientMsg::PlayerMove { id, .. } => *id,
        }
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
    InitOk(InitPlayer),
    ReconnectOk,
    InitErr(String),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OtherPlayer {
    pub username: String,
    pub location: Location,
}

/// Player initiation state that server instructs
/// the client to begin with.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct InitPlayer {
    pub id: Uuid,
    pub username: String,
    pub location: Location,
    pub hp: u32,
    pub max_hp: u32,
    pub level: u32,
    pub direction: Direction,
}

pub fn calculate_new_direction(prev: Location, target: Location) -> Direction {
    let (px, py) = prev;
    let (tx, ty) = target;

    if px == tx {
        if py < ty {
            Direction::South
        } else {
            Direction::North
        }
    } else {
        if px < tx {
            Direction::East
        } else {
            Direction::West
        }
    }
}
