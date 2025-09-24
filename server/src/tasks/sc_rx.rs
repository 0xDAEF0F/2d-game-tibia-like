use crate::{Player, Sc, ServerChannel, spawn_manager::generate_spawn_location};
use anyhow::Result;
use futures::future::join_all;
use shared::{
   Direction, GameObjects,
   network::{tcp::*, udp::*},
};
use std::{collections::HashMap, net::SocketAddr, sync::Arc};
use thin_logger::log::{debug, error, info, trace};
use tokio::{
   io::AsyncWriteExt,
   net::UdpSocket,
   sync::{Mutex, mpsc::UnboundedReceiver},
   task::JoinHandle,
};
use uuid::Uuid;

pub fn sc_rx_task(
   mut sc_rx: UnboundedReceiver<ServerChannel>,
   udp_socket: Arc<UdpSocket>,
   address_mapping: Arc<Mutex<HashMap<SocketAddr, Uuid>>>,
   players: Arc<Mutex<HashMap<Uuid, Player>>>,
   game_objects: Arc<Mutex<GameObjects>>,
) -> JoinHandle<Result<()>> {
   tokio::spawn(async move {
      while let Some(ps) = sc_rx.recv().await {
         let player_id = ps.id;
         match ps.msg {
            Sc::PlayerMove {
               client_request_id,
               location,
            } => {
               let mut players = players.lock().await;
               let player = players.get_mut(&player_id).unwrap();

               if client_request_id <= player.client_request_id {
                  trace!("received outdated player move from {}", player.username);
                  continue;
               }

               if location == player.location {
                  trace!("player {} did not move. skipping.", player.username);
                  continue;
               }

               let (old_x, old_y) = player.location;
               let (new_x, new_y) = location;
               if new_x > old_x {
                  player.direction = Direction::East;
               } else if new_x < old_x {
                  player.direction = Direction::West;
               } else if new_y > old_y {
                  player.direction = Direction::South;
               } else if new_y < old_y {
                  player.direction = Direction::North;
               }

               debug!(
                  "received player move from {} at {:?}",
                  player.username, location
               );

               debug!("player direction is: {:?}", player.direction);

               player.client_request_id = client_request_id;
               player.location = location;
            }
            Sc::MoveObject { from, to } => {
               let mut game_objects = game_objects.lock().await;
               if let Some(obj) = game_objects.0.remove(&from) {
                  debug!("moving object from {:?} to {:?}", from, to);
                  game_objects.0.insert(to, obj);
               }
            }
            Sc::Disconnect => {
               info!("{player_id} disconnected");

               let mut players = players.lock().await;
               let player = players.get(&player_id);

               let Some(player) = player else {
                  debug!("player {player_id} not found (already disconnected).");
                  continue;
               };

               let mut address_mapping = address_mapping.lock().await;

               // cleanup
               let _maybe_uuid = address_mapping.remove(&player.tcp_socket);
               let _maybe_uuid = player
                  .udp_socket
                  .and_then(|udp_socket| address_mapping.remove(&udp_socket));
               let _maybe_player = players.remove(&player_id);
            }
            Sc::ChatMsg(msg) => {
               debug!("received chat msg: \"{msg}\" from: {player_id}");

               let mut players = players.lock().await;
               let username = players.get(&player_id).unwrap().username.clone();

               // construct the message for everyone
               let chat_msg = TcpServerMsg::ChatMsg {
                  username: username.clone(),
                  msg,
               };
               let s_chat_msg = bincode::serialize(&chat_msg)?;

               let futures = players.values_mut().filter_map(|p| {
                  if p.username == username {
                     None
                  } else {
                     Some(p.tcp_tx.write_all(&s_chat_msg))
                  }
               });

               for res in join_all(futures).await {
                  if let Err(e) = res {
                     error!("failed to send chat message: {}", e);
                  }
               }
            }
            Sc::Ping(ping_id) => {
               let tcp_socket_addr = (players.lock().await).get(&player_id).unwrap().tcp_socket;

               let msg = bincode::serialize(&UdpServerMsg::Pong(ping_id))?;

               if udp_socket.send_to(&msg, tcp_socket_addr).await.is_err() {
                  error!("failed to send pong to {player_id}");
               }
            }
            Sc::Respawn => {
               info!("Player {} is respawning", player_id);

               // Use the shared spawn location generation logic
               let spawn_location =
                  generate_spawn_location(players.clone(), game_objects.clone()).await;

               let mut players = players.lock().await;
               if let Some(player) = players.get_mut(&player_id) {
                  info!(
                     "Respawning player {} at {:?}",
                     player.username, spawn_location
                  );

                  player.hp = player.max_hp;
                  player.location = spawn_location;
                  player.is_dead = false;

                  // Send respawn confirmation via TCP
                  let respawn_ok = TcpServerMsg::RespawnOk;
                  if let Ok(serialized) = bincode::serialize(&respawn_ok) {
                     if player.tcp_tx.write_all(&serialized).await.is_err() {
                        error!(
                           "Failed to send respawn confirmation to player {}",
                           player_id
                        );
                     }
                  }

                  // Send updated health and location via UDP
                  if let Some(udp_addr) = player.udp_socket {
                     let respawn_msg = UdpServerMsg::PlayerHealthUpdate { hp: player.max_hp };
                     if let Ok(serialized) = bincode::serialize(&respawn_msg) {
                        if udp_socket.send_to(&serialized, udp_addr).await.is_err() {
                           error!("Failed to send health update to player {}", player_id);
                        }
                     }
                  }

                  info!(
                     "Player {} respawned at {:?} with full health",
                     player.username, spawn_location
                  );
               } else {
                  error!("Player {} not found when attempting to respawn", player_id);
               }
            }
         }
      }
      Ok(())
   })
}
