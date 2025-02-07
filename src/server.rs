pub mod constants;
mod player;

use crate::{ClientMsg, Location};
pub use player::*;
use std::net::SocketAddr;
use uuid::Uuid;

pub enum ServerChannel {
    PlayerState {
        id: Uuid,
        client_request_id: u32,
        location: Location,
    },
    Disconnect(SocketAddr),
    MoveObject {
        from: Location,
        to: Location,
    },
    ChatMsg {
        from: SocketAddr,
        msg: String,
    },
}

impl ServerChannel {
    pub fn from_client_msg(msg: ClientMsg, addr: SocketAddr) -> Self {
        match msg {
            ClientMsg::PlayerState {
                id,
                client_request_id,
                location,
            } => ServerChannel::PlayerState {
                id,
                client_request_id,
                location,
            },
            ClientMsg::MoveObject { from, to } => ServerChannel::MoveObject { from, to },
            ClientMsg::Disconnect => ServerChannel::Disconnect(addr),
            ClientMsg::ChatMsg(msg) => ServerChannel::ChatMsg {
                from: addr,
                msg: msg.to_string(),
            },
            _ => unimplemented!(),
        }
    }
}
