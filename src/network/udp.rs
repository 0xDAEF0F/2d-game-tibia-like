use crate::{GameObjects, Location, server::Direction};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// SERVER -> CLIENT
#[derive(Debug, Serialize, Deserialize)]
pub enum UdpServerMsg {
    PlayerMove {
        location: Location,
        client_request_id: u32,
    },
    OtherPlayer {
        username: String,
        location: Location,
        direction: Direction,
    },
    Objects(GameObjects),
    Pong(u32),
}

// CLIENT -> SERVER
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
