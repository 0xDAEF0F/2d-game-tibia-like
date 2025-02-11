mod egui;
mod player;
pub mod tasks;

use crate::Location;
pub use egui::*;
pub use player::*;
use uuid::Uuid;

pub struct ClientChannel {
    pub id: Uuid,
    pub msg: Cc,
}

pub enum Cc {
    PlayerMove {
        client_request_id: u32,
        location: Location,
    },
    Disconnect,
    MoveObject {
        from: Location,
        to: Location,
    },
    ChatMsg {
        from: String,
        msg: String,
    },
    Pong(u32), // ping_id
}
