use super::Players;
use crate::constants::*;
use crate::{GameObjects, OtherPlayer, ServerMsg};
use anyhow::Result;
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
                let ps = ServerMsg::PlayerState {
                    location: player.location,
                    client_request_id: player.client_request_id,
                };
                let ser = bincode::serialize(&ps)?;
                _ = udp_socket.send_to(&ser, player_udp).await;

                let rest = players
                    .values()
                    .filter(|ps| ps.id != player.id)
                    .map(|ps| OtherPlayer {
                        username: ps.username.clone(),
                        location: ps.location,
                    });

                let rest_players = ServerMsg::RestOfPlayers(rest.collect());
                let rest_players_ser = bincode::serialize(&rest_players)?;
                _ = udp_socket.send_to(&rest_players_ser, player_udp).await;

                let objects = ServerMsg::Objects(game_objects.clone());
                let encoded_objects = bincode::serialize(&objects)?;
                _ = udp_socket.send_to(&encoded_objects, player_udp).await;
            }
        }
    })
}
