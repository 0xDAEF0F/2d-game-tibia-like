use crate::Player;
use shared::{
   GameObjects, Location,
   constants::{MAP_HEIGHT, MAP_WIDTH},
};
use std::{collections::HashMap, sync::Arc};
use thin_logger::log::{info, warn};
use tokio::sync::Mutex;
use uuid::Uuid;

pub async fn generate_spawn_location(
   players: Arc<Mutex<HashMap<Uuid, Player>>>,
   game_objects: Arc<Mutex<GameObjects>>,
) -> Location {
   let players = players.lock().await;
   let mut taken_locations = players.values().map(|p| p.location).collect::<Vec<_>>();

   // Add monster locations
   let game_objs = game_objects.lock().await;
   for (location, obj) in game_objs.0.iter() {
      if obj.id() == 63
      /* orc */
      {
         taken_locations.push(*location);
      }
   }
   drop(game_objs);

   // Find first available location starting from (0,0)
   let mut y = 0;
   while y < MAP_HEIGHT {
      let mut x = 0;
      while x < MAP_WIDTH {
         let test_loc = (x, y, 0); // Always spawn at z_level 0
         if !taken_locations.contains(&test_loc) {
            info!("Found spawn location for new player at {:?}", test_loc);
            return test_loc;
         }
         x += 1;
      }
      y += 1;
   }

   // Fallback to (0,0) if somehow all locations are taken
   warn!("No available spawn location found, using (0,0)");
   (0, 0, 0)
}
