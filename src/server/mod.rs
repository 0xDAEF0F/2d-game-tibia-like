mod player;
pub mod tasks;

use crate::Location;
pub use player::*;
use uuid::Uuid;

pub struct ServerChannel {
    id: Uuid,
    msg: Sc,
}

pub enum Sc {
    PlayerMove {
        client_request_id: u32,
        location: Location,
    },
    Disconnect,
    MoveObject {
        from: Location,
        to: Location,
    },
    ChatMsg(String), // message
    Ping(u32),       // ping_id
}
