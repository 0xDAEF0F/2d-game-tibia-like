mod egui;
mod player;
pub mod tasks;

use crate::{GameObjects, Location, OtherPlayer};
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
    RestOfPlayers(Vec<OtherPlayer>),
    Disconnect,
    MoveObject {
        from: Location,
        to: Location,
    },
    Objects(GameObjects),
    ChatMsg {
        from: String,
        msg: String,
    },
    Pong(u32), // ping_id
}
