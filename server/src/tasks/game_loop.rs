use crate::{MapElement, MmoMap, Player, player::DamageResult};
use anyhow::Result;
use futures::future::join_all;
use shared::{
   GameObject, GameObjects, Location,
   constants::*,
   network::{sendable::SendableAsync, udp::*},
};
use std::{
   collections::HashMap,
   net::SocketAddr,
   sync::Arc,
   time::{Duration, Instant},
};
use thin_logger::log::{debug, error, info, trace};
use tokio::{net::UdpSocket, sync::Mutex, task::JoinHandle};
use uuid::Uuid;

// ================ Helper Functions ================

fn is_adjacent(a: Location, b: Location) -> bool {
   let dx = (a.0 as i32 - b.0 as i32).abs();
   let dy = (a.1 as i32 - b.1 as i32).abs();
   dx <= 1 && dy <= 1
}

fn is_within_view(monster_pos: Location, player_pos: Location) -> bool {
   let min_x = (monster_pos.0 as i32) - ((CAMERA_WIDTH / 2) as i32);
   let max_x = (monster_pos.0 as i32) + ((CAMERA_WIDTH / 2) as i32);
   let min_y = (monster_pos.1 as i32) - ((CAMERA_HEIGHT / 2) as i32);
   let max_y = (monster_pos.1 as i32) + ((CAMERA_HEIGHT / 2) as i32);

   (min_x..=max_x).contains(&(player_pos.0 as i32))
      && (min_y..=max_y).contains(&(player_pos.1 as i32))
}

fn get_active_monsters(game_objects: &GameObjects) -> Vec<(Location, GameObject)> {
   game_objects
      .0
      .clone()
      .into_iter()
      .filter(|(_, obj)| obj.id() == 63) // orc
      .collect()
}

// ================ Player Damage Handling ================

async fn handle_player_damage(
   player: &mut Player,
   damage: u32,
   game_objects: &mut GameObjects,
   udp_socket: &UdpSocket,
   player_udp: SocketAddr,
) {
   match player.take_damage(damage) {
      DamageResult::AlreadyDead => {}
      DamageResult::Damaged { damage, hp } => {
         info!(
            "Player {} took {} damage. HP: {}/{}",
            player.username, damage, hp, player.max_hp
         );

         let health_msg = UdpServerMsg::PlayerHealthUpdate { hp };
         udp_socket
            .send_msg_and_log_(health_msg, Some(player_udp))
            .await;
      }
      DamageResult::Died {
         damage,
         death_message,
      } => {
         info!(
            "Player {} has died from {} damage.",
            player.username, damage
         );

         // Place a flowerpot at the player's death location (simulating a corpse)
         game_objects.0.insert(
            player.location,
            GameObject::FlowerPot {
               id: 149,
               tileset_location: 0,
            },
         );

         let death_msg = UdpServerMsg::PlayerDeath {
            message: death_message,
         };
         udp_socket
            .send_msg_and_log_(death_msg, Some(player_udp))
            .await;
      }
   }
}

// ================ Monster AI ================

async fn process_monster_attack(
   monster_location: Location,
   player: &mut Player,
   game_objects: &mut GameObjects,
   mmo_map: &Arc<Mutex<MmoMap>>,
   udp_socket: &UdpSocket,
   player_udp: SocketAddr,
) -> bool {
   let mut mmo_map = mmo_map.lock().await;

   let MapElement::Monster(mut monster) = mmo_map[monster_location] else {
      debug!("Invalid monster location for attack");
      return false;
   };

   if monster.last_attack.elapsed() < Duration::from_secs(2) {
      trace!("Monster can't attack yet (cooldown)");
      return false;
   }

   trace!("Monster is adjacent to player. Attacking!");

   // Update monster's last attack time
   monster.last_attack = Instant::now();
   mmo_map[monster_location] = MapElement::Monster(monster);

   // Release the lock before handling damage
   drop(mmo_map);

   // hardcoded damage for now
   handle_player_damage(player, 50, game_objects, udp_socket, player_udp).await;

   true
}

async fn process_monster_movement(
   monster_location: Location,
   player_location: Location,
   game_objects: &mut GameObjects,
   mmo_map: &Arc<Mutex<MmoMap>>,
) -> Result<()> {
   let mut mmo_map = mmo_map.lock().await;

   let shortest_path = mmo_map.shortest_path(monster_location, player_location);

   if shortest_path.len() <= 2 {
      trace!("No valid path to player or already adjacent");
      return Ok(());
   }

   let next_position = shortest_path[1];

   let MapElement::Monster(monster) = &mmo_map[monster_location] else {
      debug!("Invalid monster location");
      return Ok(());
   };

   if monster.last_movement.elapsed() < Duration::from_millis(200) {
      trace!("Monster cant move yet (cooldown)");
      return Ok(());
   }

   if game_objects
      .move_object(monster_location, next_position)
      .is_none()
   {
      return Err(anyhow::anyhow!("Failed to move monster"));
   }

   mmo_map.move_monster(monster_location, next_position);
   Ok(())
}

async fn process_monster_ai(
   player: &mut Player,
   game_objects: &mut GameObjects,
   mmo_map: &Arc<Mutex<MmoMap>>,
   udp_socket: &UdpSocket,
   player_udp: SocketAddr,
) -> Result<()> {
   if player.is_dead {
      return Ok(());
   }

   let monsters = get_active_monsters(game_objects);

   for (monster_location, _) in monsters {
      if !is_within_view(monster_location, player.location) {
         continue;
      }

      trace!("Monster can see player {}", player.username);

      if is_adjacent(monster_location, player.location) {
         process_monster_attack(
            monster_location,
            player,
            game_objects,
            mmo_map,
            udp_socket,
            player_udp,
         )
         .await;
      } else {
         process_monster_movement(monster_location, player.location, game_objects, mmo_map).await?;
      }
   }

   Ok(())
}

// ================ Player Updates ================

async fn send_player_updates(
   player_id: Uuid,
   player: &Player,
   all_players: &HashMap<Uuid, Player>,
   game_objects: &GameObjects,
   udp_socket: &UdpSocket,
) {
   let Some(player_udp) = player.udp_socket else {
      return;
   };

   // Send player's own position update
   if !player.is_dead {
      let ps = UdpServerMsg::PlayerMove {
         location: player.location,
         client_request_id: player.client_request_id,
      };
      udp_socket.send_msg_and_log_(ps, Some(player_udp)).await;
   }

   // Send other players' positions
   let other_players_futures = all_players
      .values()
      .filter(|&ps| ps.id != player_id && !ps.is_dead)
      .map(|ps| {
         udp_socket.send_msg_and_log_(
            UdpServerMsg::OtherPlayer {
               username: ps.username.clone(),
               location: ps.location,
               direction: ps.direction,
            },
            Some(player_udp),
         )
      });
   join_all(other_players_futures).await;

   // Send game objects
   let objects = UdpServerMsg::Objects(game_objects.clone());
   udp_socket
      .send_msg_and_log_(objects, Some(player_udp))
      .await;
}

// ================ Main Game Loop ================

async fn process_game_tick(
   udp_socket: &Arc<UdpSocket>,
   players: &Arc<Mutex<HashMap<Uuid, Player>>>,
   game_objects: &Arc<Mutex<GameObjects>>,
   mmo_map: &Arc<Mutex<MmoMap>>,
) -> Result<()> {
   let mut players_guard = players.lock().await;
   let mut game_objects = game_objects.lock().await;

   let player_ids: Vec<Uuid> = players_guard.keys().copied().collect();

   for player_id in player_ids {
      // Process monster AI for this player
      {
         let player = match players_guard.get_mut(&player_id) {
            Some(p) => p,
            None => continue,
         };

         let Some(player_udp) = player.udp_socket else {
            continue;
         };

         process_monster_ai(player, &mut game_objects, mmo_map, udp_socket, player_udp).await?;
      }

      // Send updates to this player
      {
         let player = match players_guard.get(&player_id) {
            Some(p) => p,
            None => continue,
         };

         send_player_updates(player_id, player, &players_guard, &game_objects, udp_socket).await;
      }
   }

   Ok(())
}

/// This task should never finish. If it does, it must be an error.
pub fn game_loop_task(
   udp_socket: Arc<UdpSocket>,
   players: Arc<Mutex<HashMap<Uuid, Player>>>,
   game_objects: Arc<Mutex<GameObjects>>,
   mmo_map: Arc<Mutex<MmoMap>>,
) -> JoinHandle<Result<()>> {
   tokio::spawn(async move {
      let mut interval = tokio::time::interval(Duration::from_millis(SERVER_TICK_RATE));

      loop {
         interval.tick().await;

         if let Err(e) = process_game_tick(&udp_socket, &players, &game_objects, &mmo_map).await {
            error!("Game tick failed: {}", e);
            return Err(e);
         }
      }
   })
}

