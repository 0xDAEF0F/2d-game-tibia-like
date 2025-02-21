pub mod client;
pub mod constants;
mod game_objects;
mod logger;
mod network;
pub mod server;
mod tilesheet;
mod utils;

pub use game_objects::*;
pub use logger::*;
pub use network::*;
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
