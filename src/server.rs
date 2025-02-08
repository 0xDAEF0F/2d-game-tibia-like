pub mod constants;
mod player;

use crate::Location;
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
    // TODO: which object?
    MoveObject {
        from: Location,
        to: Location,
    },
    ChatMsg {
        from: SocketAddr,
        msg: String,
    },
}
