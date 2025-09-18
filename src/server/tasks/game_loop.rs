use super::Players;
use crate::{
	GameObjects,
	constants::*,
	sendable::SendableAsync,
	server::{MapElement, MmoMap},
	udp::*,
};
use anyhow::Result;
use futures::future::join_all;
use itertools::Itertools;
use log::{debug, error, trace};
use std::{sync::Arc, time::{Duration, Instant}};
use tokio::{net::UdpSocket, sync::Mutex, task::JoinHandle};
use uuid::Uuid;

type Udp = Arc<UdpSocket>;
type Objects = Arc<Mutex<GameObjects>>;

/// This task should never finish. If it does, it must be an error.
pub fn game_loop_task(
	udp_socket: Udp,
	players: Players,
	game_objects: Objects,
	mmo_map: Arc<Mutex<MmoMap>>,
) -> JoinHandle<Result<()>> {
	tokio::spawn(async move {
		let mut interval = tokio::time::interval(Duration::from_millis(SERVER_TICK_RATE));
		loop {
			interval.tick().await;

			let mut players = players.lock().await;
			let mut game_objects = game_objects.lock().await;

			// Collect player IDs to process
			let player_ids: Vec<Uuid> = players.keys().copied().collect();

			for player_id in player_ids {
				let player = match players.get_mut(&player_id) {
					Some(p) => p,
					None => continue,
				};

				let Some(player_udp) = player.udp_socket else {
					continue;
				};

				// does the player have a monster within view?
				let monsters = game_objects
					.0
					.clone()
					.into_iter()
					.filter(|(_, obj)| obj.id() == 63 /* orc */)
					.collect_vec();

				for (monst_location @ (x, y), _monster) in monsters {
					let min_x = (x as i32) - ((CAMERA_WIDTH / 2) as i32);
					let max_x = (x as i32) + ((CAMERA_WIDTH / 2) as i32);
					let min_y = (y as i32) - ((CAMERA_HEIGHT / 2) as i32);
					let max_y = (y as i32) + ((CAMERA_HEIGHT / 2) as i32);
					if (min_x..=max_x).contains(&(player.location.0 as i32))
						&& (min_y..=max_y).contains(&(player.location.1 as i32))
					{
						trace!("Monster can see player {}", player.username);

						// TODO: move elsewhere
						fn is_adjacent(a: (i32, i32), b: (i32, i32)) -> bool {
							(a.0 - b.0).abs() <= 1 && (a.1 - b.1).abs() <= 1
						}
						if is_adjacent(
							(x as i32, y as i32),
							(player.location.0 as i32, player.location.1 as i32),
						) {
							// Check if monster can attack (2 second cooldown)
							let mut mmo_map = mmo_map.lock().await;
							let MapElement::Monster(mut monster) = mmo_map[monst_location]
							else {
								debug!("Invalid monster location for attack");
								continue;
							};

							if monster.last_attack.elapsed() < Duration::from_secs(2) {
								trace!("Monster can't attack yet (cooldown)");
								continue;
							}

							trace!("Monster is adjacent to player. Attacking!");
							// Apply damage to player
							player.take_damage(1);
							log::info!(
								"Player {} took 1 damage. HP: {}/{}",
								player.username,
								player.hp,
								player.max_hp
							);

							// Update monster's last attack time
							monster.last_attack = Instant::now();
							mmo_map[monst_location] = MapElement::Monster(monster);

							// Send health update to client
							let health_msg = UdpServerMsg::PlayerHealthUpdate {
								hp: player.hp,
							};
							udp_socket
								.send_msg_and_log_(health_msg, Some(player_udp))
								.await;
							continue;
						}

						let mut mmo_map = mmo_map.lock().await;

						let shortest_path =
							mmo_map.shortest_path(monst_location, player.location);
						let shortest_path = &shortest_path[1..&shortest_path.len() - 1];

						if shortest_path.is_empty() {
							trace!("No path to player");
							continue;
						}

						let MapElement::Monster(monster) = &mmo_map[monst_location]
						else {
							debug!("Invalid monster location");
							continue;
						};

						if monster.last_movement.elapsed() < Duration::from_millis(200) {
							log::trace!("Monster cant move yet (cooldown)");
							continue;
						}

						if game_objects
							.move_object(monst_location, shortest_path[0])
							.is_none()
						{
							error!("Failed to move monster");
							std::process::exit(1);
						};

						mmo_map.move_monster(monst_location, shortest_path[0]);
					}
				}

				let ps = UdpServerMsg::PlayerMove {
					location: player.location,
					client_request_id: player.client_request_id,
				};
				udp_socket.send_msg_and_log_(ps, Some(player_udp)).await;

				let rest_players_udp_msg_future =
					players.values().filter(|&ps| ps.id != player_id).map(|ps| {
						udp_socket.send_msg_and_log_(
							UdpServerMsg::OtherPlayer {
								username: ps.username.clone(),
								location: ps.location,
								direction: ps.direction,
							},
							Some(player_udp),
						)
					});
				join_all(rest_players_udp_msg_future).await;

				let objects = UdpServerMsg::Objects(game_objects.clone());
				(udp_socket.send_msg_and_log_(objects, Some(player_udp))).await;
			}
		}
	})
}
