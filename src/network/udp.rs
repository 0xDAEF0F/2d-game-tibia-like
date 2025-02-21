use serde::{Deserialize, Serialize};
use crate::{GameObjects, Location, OtherPlayer};
use uuid::Uuid;

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