use crate::{
    GameObjects, TcpServerMsg, UdpServerMsg,
    server::{Player, Sc, ServerChannel},
};
use anyhow::Result;
use futures::future::join_all;
use log::{debug, error, info, trace, warn};
use std::{collections::HashMap, net::SocketAddr, sync::Arc};
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

                    debug!(
                        "received player move from {} at {:?}",
                        player.username, location
                    );

                    // TODO: check if location is valid
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
                    let tcp_socket_addr =
                        (players.lock().await).get(&player_id).unwrap().tcp_socket;

                    let msg = bincode::serialize(&UdpServerMsg::Pong(ping_id))?;

                    if udp_socket.send_to(&msg, tcp_socket_addr).await.is_err() {
                        error!("failed to send pong to {player_id}");
                    }
                }
            }
        }
        Ok(())
    })
}
