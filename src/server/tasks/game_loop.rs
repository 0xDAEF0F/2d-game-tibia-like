use super::Players;
use crate::constants::*;
use crate::{GameObjects, OtherPlayer, UdpServerMsg};
use anyhow::Result;
use itertools::Itertools;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::UdpSocket;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;

type Udp = Arc<UdpSocket>;
type Objects = Arc<Mutex<GameObjects>>;

/// This task should never finish. If it does, it must be an error.
pub fn game_loop_task(
    udp_socket: Udp,
    players: Players,
    game_objects: Objects,
) -> JoinHandle<Result<()>> {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_millis(SERVER_TICK_RATE));
        loop {
            interval.tick().await;

            let players = players.lock().await;
            let game_objects = game_objects.lock().await;

            for player in players.values() {
                let Some(player_udp) = player.udp_socket else {
                    continue;
                };

                // does the player have a monster within view?
                let monsters = game_objects
                    .0
                    .iter()
                    .filter(|(_, obj)| obj.id() == 63 /* orc */)
                    .collect_vec();

                for (&(x, y), monster) in monsters {
                    let min_x = (x as i32) - ((CAMERA_WIDTH / 2) as i32);
                    let max_x = (x as i32) + ((CAMERA_WIDTH / 2) as i32);
                    let min_y = (y as i32) - ((CAMERA_HEIGHT / 2) as i32);
                    let max_y = (y as i32) + ((CAMERA_HEIGHT / 2) as i32);
                    if (min_x..=max_x).contains(&(player.location.0 as i32))
                        && (min_y..=max_y).contains(&(player.location.1 as i32))
                    {
                        println!("Player {} can see monster at {:?}", player.username, (x, y));
                    }
                }

                let ps = UdpServerMsg::PlayerMove {
                    location: player.location,
                    client_request_id: player.client_request_id,
                };
                let ser = bincode::serialize(&ps)?;
                _ = udp_socket.send_to(&ser, player_udp).await;

                let rest = players.values().filter_map(|ps| {
                    ps.id.ne(&player.id).then(|| OtherPlayer {
                        username: ps.username.clone(),
                        location: ps.location,
                    })
                });

                let rest_players = UdpServerMsg::RestOfPlayers(rest.collect());
                let rest_players_ser = bincode::serialize(&rest_players)?;
                _ = udp_socket.send_to(&rest_players_ser, player_udp).await;

                let objects = UdpServerMsg::Objects(game_objects.clone());
                let encoded_objects = bincode::serialize(&objects)?;
                _ = udp_socket.send_to(&encoded_objects, player_udp).await;
            }
        }
    })
}
