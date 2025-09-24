pub mod constants;
pub mod game_objects;
pub mod logger;
pub mod network;

pub use game_objects::*;
pub use logger::*;
pub use network::*;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use uuid::Uuid;

pub type Location = (u32, u32); // (x, y) coordinates

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
   North,
   South,
   East,
   West,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OtherPlayer {
   pub username: String,
   pub location: Location,
   pub direction: Direction,
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

   // NOTE: there may be a bug here
   match px.cmp(&tx) {
      Ordering::Equal => match py.cmp(&ty) {
         Ordering::Less => Direction::South,
         _ => Direction::North,
      },
      Ordering::Less => Direction::East,
      Ordering::Greater => Direction::West,
   }
}
