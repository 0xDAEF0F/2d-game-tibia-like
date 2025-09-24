pub mod egui;
pub mod movement;
pub mod object_interaction;
pub mod pathfinding;
pub mod player;
pub mod rendering;
pub mod tasks;
pub mod tilesheet;
pub mod utils;

pub use egui::*;
pub use player::{ClientOtherPlayer as OtherPlayer, OtherPlayers, Player};
use shared::{GameObjects, Location};
pub use tilesheet::MmoTilesheets;
pub use utils::{FpsLogger, PingMonitor};
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
   OtherPlayer(shared::OtherPlayer),
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
   ReconnectOk,
   PlayerHealthUpdate {
      hp: u32,
   },
   PlayerDeath {
      message: String,
   },
   RespawnOk {
      hp: u32,
      location: Location,
   },
}
