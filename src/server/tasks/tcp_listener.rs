use super::Players;
use crate::server::{Direction, Player, Sc, ServerChannel};
use crate::{InitPlayer, TcpClientMsg, TcpServerMsg};
use anyhow::{Context, Result, bail};
use log::{debug, error, info, trace, warn};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::tcp::OwnedReadHalf;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{Mutex, mpsc::UnboundedSender};
use tokio::task::JoinHandle;
use tokio_stream::StreamExt;
use tokio_stream::wrappers::TcpListenerStream;
use uuid::Uuid;

pub fn tcp_listener_task(
    tcp_listener: TcpListener,
    players: Players,
    address_mapping: Arc<Mutex<HashMap<SocketAddr, Uuid>>>,
    sc_tx: UnboundedSender<ServerChannel>,
) -> JoinHandle<Result<()>> {
    tokio::spawn(async move {
        let mut iter = TcpListenerStream::new(tcp_listener);

        while let Ok(tcp_stream) = iter.next().await.context("stream ended")? {
            info!(
                "accepted TCP connection from: {:?}. authenticating...",
                tcp_stream.peer_addr()
            );

            handle_tcp_stream(
                tcp_stream,
                players.clone(),
                address_mapping.clone(),
                sc_tx.clone(),
            );
        }

        Ok(())
    })
}

fn handle_tcp_stream(
    mut stream: TcpStream,
    players: Players,
    address_mapping: Arc<Mutex<HashMap<SocketAddr, Uuid>>>,
    sc_tx: UnboundedSender<ServerChannel>,
) {
    // this task does not block the server and it can continue
    // accepting new connections.
    tokio::spawn(async move {
        let user_address = stream.peer_addr().expect("expect to have the user address");

        // if it fails to do so (auth) this task will be exited
        let auth_type = match authenticate_tcp_client(&mut stream, players.clone()).await {
            Ok(u) => u,
            Err(e) => {
                error!("failed to authenticate {user_address}: {e}");
                return;
            }
        };

        if let AuthType::Connection(username) = auth_type {
            let (tcp_read, mut tcp_write) = stream.into_split();

            let init_player = InitPlayer {
                id: Uuid::new_v4(),
                username: username.clone(),
                location: (0, 0),
                hp: 100,
                max_hp: 100,
                level: 1,
                direction: Direction::South,
            };

            let ser = bincode::serialize(&TcpServerMsg::InitOk(init_player.clone())).unwrap();
            if tcp_write.write_all(&ser).await.is_err() {
                error!("failed to send init ok to user: {username}");
                return;
            };

            let new_player = Player::new(init_player.id, username, user_address, tcp_write);

            // storage
            address_mapping
                .lock()
                .await
                .insert(user_address, init_player.id);
            players.lock().await.insert(init_player.id, new_player);

            // set up tcp reader
            setup_tcp_reader(tcp_read, sc_tx.clone(), address_mapping.clone());
        } else if let AuthType::Reconnection(uuid) = auth_type {
            debug!("reconnecting player: {uuid}");
            let (tcp_read, mut tcp_write) = stream.into_split();

            let ser = bincode::serialize(&TcpServerMsg::ReconnectOk).unwrap();
            if tcp_write.write_all(&ser).await.is_err() {
                error!("failed to send reconnect ok to user");
                return;
            };

            // storage
            address_mapping.lock().await.insert(user_address, uuid);
            let mut players = players.lock().await;
            let player = players.get_mut(&uuid).unwrap();

            player.tcp_tx = tcp_write;
            player.tcp_socket = user_address;

            // set up tcp reader
            setup_tcp_reader(tcp_read, sc_tx.clone(), address_mapping.clone());
        }
    });
}

enum AuthType {
    Reconnection(Uuid),
    Connection(String),
}

async fn authenticate_tcp_client(tcp_stream: &mut TcpStream, players: Players) -> Result<AuthType> {
    let mut buf = [0; 1024];

    let size = tcp_stream.read(&mut buf).await?;
    let c_msg: TcpClientMsg = bincode::deserialize(&buf[..size])?;

    let username = match c_msg {
        TcpClientMsg::Init(username) => username,
        TcpClientMsg::Reconnect(uuid) => {
            let mut players = players.lock().await;
            let Some(p) = players.get_mut(&uuid) else {
                bail!("player does not exist");
            };
            p.tcp_socket = tcp_stream.peer_addr()?;
            return Ok(AuthType::Reconnection(uuid));
        }
        _ => bail!("invalid client message"),
    };

    println!("submitted username is: {}", username);

    let is_username_taken = (players.lock().await)
        .values()
        .any(|p| p.username == username);

    if is_username_taken {
        let str = format!("username: {} is taken.", username);

        info!("{}", str);

        // send error to client
        let msg = TcpServerMsg::InitErr(str);
        let s_msg = bincode::serialize(&msg).unwrap();
        _ = tcp_stream.write(&s_msg).await;

        bail!("username: {username} is taken.");
    }

    Ok(AuthType::Connection(username))
}

/// Spins up a task to listen to incoming TCP messages
/// and relays them to the server channel.
fn setup_tcp_reader(
    mut tcp_read: OwnedReadHalf,
    sc_tx: UnboundedSender<ServerChannel>,
    address_mapping: Arc<Mutex<HashMap<SocketAddr, Uuid>>>,
) {
    tokio::spawn(async move {
        let peer_addr = tcp_read.peer_addr().unwrap();
        let mut buffer = [0; 1024];
        loop {
            match tcp_read.read(&mut buffer).await {
                Ok(n) if n > 0 => {
                    let Ok(msg) = bincode::deserialize::<TcpClientMsg>(&buffer[..n]) else {
                        error!("could not deserialize msg from client. closing connection.");
                        break;
                    };

                    trace!("received TCP msg from {peer_addr:?}");

                    let user_id = *address_mapping.lock().await.get(&peer_addr).unwrap();

                    let sc = match msg {
                        TcpClientMsg::ChatMsg(m) => Sc::ChatMsg(m),
                        TcpClientMsg::Disconnect => Sc::Disconnect,
                        TcpClientMsg::Ping(p_id) => Sc::Ping(p_id),
                        _ => {
                            warn!("unwanted msg: {msg:?}. skipping...");
                            continue;
                        }
                    };

                    let sc = ServerChannel {
                        id: user_id,
                        msg: sc,
                    };

                    _ = sc_tx.send(sc);
                }
                _ => {
                    info!("{:?} closed TCP connnection or tcp read failed.", peer_addr);

                    let Some(user_id) = address_mapping.lock().await.get(&peer_addr).copied()
                    else {
                        break;
                    };

                    let disconnect = ServerChannel {
                        id: user_id,
                        msg: Sc::Disconnect,
                    };

                    _ = sc_tx.send(disconnect);
                }
            }
        }

        // cleanup
    });
}
