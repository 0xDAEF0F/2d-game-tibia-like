pub mod constants;
mod player;

use crate::{ClientMsg, Location, PlayerState};
use std::net::SocketAddr;

pub enum ServerChannel {
    PlayerState(PlayerState),
    Disconnect(SocketAddr),
    MoveObject { from: Location, to: Location },
    ChatMsg { from: SocketAddr, msg: String },
}

impl ServerChannel {
    pub fn from_client_msg(msg: ClientMsg, addr: SocketAddr) -> Self {
        match msg {
            ClientMsg::PlayerState(ps) => ServerChannel::PlayerState(ps),
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
