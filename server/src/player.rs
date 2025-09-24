use shared::{Direction, Location};
use std::net::SocketAddr;
use tokio::net::tcp::OwnedWriteHalf;
use uuid::Uuid;

#[derive(Debug)]
pub struct Player {
   pub id: Uuid,
   pub username: String,
   pub client_request_id: u32,
   pub location: Location,

   pub hp: u32,
   pub max_hp: u32,
   pub level: u32,
   pub direction: Direction,
   pub is_dead: bool,

   pub tcp_tx: OwnedWriteHalf,
   pub tcp_socket: SocketAddr,
   pub udp_socket: Option<SocketAddr>,
}

pub enum DamageResult {
   Damaged { damage: u32, hp: u32 },
   Died { damage: u32, death_message: String },
   AlreadyDead,
}

impl Player {
   pub fn take_damage(&mut self, damage: u32) -> DamageResult {
      if self.is_dead {
         return DamageResult::AlreadyDead;
      }

      self.hp = self.hp.saturating_sub(damage);

      if self.hp == 0 {
         self.is_dead = true;
         DamageResult::Died {
            damage,
            death_message: "You have been slain!".to_string(),
         }
      } else {
         DamageResult::Damaged {
            damage,
            hp: self.hp,
         }
      }
   }
}
