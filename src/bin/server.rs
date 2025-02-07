use anyhow::{Context, Result, bail};
use egui_macroquad::macroquad::prelude::warn;
use itertools::Itertools;
use log::{debug, error, info, trace};
use my_mmo::server::Player;
use my_mmo::server::ServerChannel;
use my_mmo::server::constants::*;
use my_mmo::*;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::{TcpListener, TcpStream, UdpSocket};
use tokio::sync::mpsc::UnboundedSender;
use tokio::sync::{Mutex, mpsc};
use tokio::task::JoinHandle;
use tokio::time::{self, Duration};
use tokio_stream::StreamExt;
use uuid::Uuid;

// TODO: the server is supposed to map tcp/udp clients from their socket
// addresses to their player_id/username.

// 1. Client establishes connection with server through TCP
//    and server sends a session id to the client
// 2. When client sends his UDP datagram he also sends the sessions id
//    so server can link both addresses.
// 3. If the client disconnects through TCP. server destroys both UDP
//    and TCP as well as information that is no longer needed about the player.
// 4. If the client stops sending keep alives or pings for x amount of time
//    the server destroys the user session

#[tokio::main]
async fn main() -> Result<()> {
    MmoLogger::init("debug");

    let udp_socket = Arc::new(UdpSocket::bind(SERVER_UDP_ADDR).await?);
    info!("Server listening on UDP: {}", SERVER_UDP_ADDR);

    let tcp_listener = TcpListener::bind(SERVER_TCP_ADDR).await?;
    info!("Server listening on TCP: {}", SERVER_TCP_ADDR);

    let (server_channel_sender, mut server_channel_receiver) =
        mpsc::unbounded_channel::<ServerChannel>();

    let address_mapping: HashMap<SocketAddr, Uuid> = HashMap::new(); // addr -> player_id
    let address_mapping = Arc::new(Mutex::new(address_mapping));

    let players = HashMap::<Uuid, Player>::new();
    let players = Arc::new(Mutex::new(players));

    let game_objects = GameObjects::new();
    let game_objects = Arc::new(Mutex::new(game_objects));

    let tcp_writers_pool: HashMap<SocketAddr, OwnedWriteHalf> = HashMap::new();
    let tcp_writers_pool = Arc::new(Mutex::new(tcp_writers_pool));

    // Accepts TCP connections. Depends on `tcp_listener` and `tx_`.
    let tcp_writers_pool_clone = Arc::clone(&tcp_writers_pool);
    let players_clone = Arc::clone(&players);
    let server_channel_sender_clone = server_channel_sender.clone();
    let task1_handle = tokio::spawn(async move {
        use tokio_stream::wrappers::TcpListenerStream;
        let mut iter = TcpListenerStream::new(tcp_listener);

        while let Ok(tcp_stream) = iter.next().await.context("stream ended")? {
            let peer_addr = tcp_stream.peer_addr()?;

            info!("accepted TCP connection from: {peer_addr}",);

            // currently only validates the username of the client.
            validate_client(
                tcp_stream,
                tcp_writers_pool_clone.clone(),
                players_clone.clone(),
                server_channel_sender_clone.clone(),
            );
        }

        anyhow::Ok(())
    });

    // game loop. depends on `rx`, `players`, `game_objects`, and `udp_socket`.
    let players_clone = players.clone();
    let objects_clone = game_objects.clone();
    let udp_socket_ = udp_socket.clone();
    let task2_handle: JoinHandle<Result<()>> = tokio::spawn(async move {
        let mut interval = time::interval(Duration::from_millis(SERVER_TICK_RATE));
        loop {
            interval.tick().await;
            let players = players_clone.lock().await;
            let game_objects = objects_clone.lock().await;

            for player in players.values() {
                let Some(udp_socket) = player.udp_socket else {
                    continue;
                };
                let ps = ServerMsg::PlayerState {
                    location: player.location,
                    client_request_id: player.client_request_id,
                };
                let ser = bincode::serialize(&ps)?;
                _ = udp_socket_.send_to(&ser, udp_socket).await;

                let rest = players
                    .values()
                    .filter(|ps| ps.id != player.id)
                    .map(|ps| OtherPlayer {
                        username: ps.username.clone(),
                        location: ps.location,
                    });

                let rest_players = ServerMsg::RestOfPlayers(rest.collect());
                let rest_players_ser = bincode::serialize(&rest_players)?;
                _ = udp_socket_.send_to(&rest_players_ser, udp_socket).await;

                let objects = ServerMsg::Objects(game_objects.clone());
                let encoded_objects = bincode::serialize(&objects)?;
                _ = udp_socket_.send_to(&encoded_objects, udp_socket).await;
            }
        }
    });

    // Receives UDP msgs from clients. depends on the UDP socket and server channel sender.
    let udp_socket_ = udp_socket.clone();
    let task3_handle = tokio::spawn(async move {
        let mut buf = [0; 1024];
        while let Ok((size, src)) = udp_socket_.recv_from(&mut buf).await {
            // we need to retrieve a user id from the socket address
            // can the user just tell us his user id?

            let Ok(msg) = bincode::deserialize::<ClientMsg>(&buf[..size]) else {
                debug!("failed to deserialize message from {src:?}");
                continue;
            };

            if let ClientMsg::Ping(x) = &msg {
                let pong = bincode::serialize(&ServerMsg::Pong(*x)).unwrap();
                let _result = udp_socket_.try_send_to(&pong, src);
                trace!("sending pong for {:?}", *x);
                continue;
            }

            let sc = match msg {
                ClientMsg::Disconnect => ServerChannel::Disconnect(src),
                ClientMsg::PlayerState {
                    id,
                    client_request_id,
                    location,
                } => ServerChannel::PlayerState {
                    id,
                    client_request_id,
                    location,
                },
                ClientMsg::MoveObject { from, to } => ServerChannel::MoveObject { from, to },
                ClientMsg::ChatMsg(_) => unimplemented!("chat messages only allowed through TCP"),
                ClientMsg::Ping(_) => unreachable!(),
                ClientMsg::Init(_) => unreachable!(),
            };

            _ = server_channel_sender.send(sc);
        }

        anyhow::Ok(())
    });

    let task4_handle = tokio::spawn(async move {
        // Receives messages from the server channel sender. depends on `rx`.
        while let Some(ps) = server_channel_receiver.recv().await {
            match ps {
                ServerChannel::PlayerState {
                    id,
                    client_request_id,
                    location,
                } => {
                    let mut players = players.lock().await;

                    let Some(player) = players.get_mut(&id) else {
                        error!("player does not yet exist");
                        continue;
                    };

                    if client_request_id <= player.client_request_id {
                        trace!("received outdated player state from {}", player.username);
                        continue;
                    }

                    if location == player.location {
                        trace!("player {} did not move. skipping.", player.username);
                        continue;
                    }

                    debug!(
                        "received player state from {} at {:?}",
                        player.username, location
                    );

                    // TODO: check if location is valid
                    player.client_request_id = client_request_id;
                    player.location = location;
                }
                ServerChannel::MoveObject { from, to } => {
                    let mut game_objects = game_objects.lock().await;
                    if let Some(obj) = game_objects.0.remove(&from) {
                        debug!("moving object from {:?} to {:?}", from, to);
                        game_objects.0.insert(to, obj);
                    }
                }
                // TODO: this arm needs further revisions
                ServerChannel::Disconnect(addr) => {
                    info!("{addr:?} disconnected");

                    let mut address_mapping = address_mapping.lock().await;
                    let Some(user_id) = address_mapping.remove(&addr) else {
                        warn!("{} was not in the address lookup table", addr);
                        continue;
                    };

                    let mut players = players.lock().await;
                    let Some(player) = players.remove(&user_id) else {
                        warn!("user {} was not in the players map", user_id);
                        continue;
                    };

                    let mut pool = tcp_writers_pool.lock().await;
                    let Some(tcp_writer) = pool.remove(&addr) else {
                        warn!("user {} was not in the writers pool", user_id);
                        continue;
                    };
                }
                ServerChannel::ChatMsg { from, msg } => {
                    debug!("received chat msg: \"{}\" from: {}", msg, from);

                    // relay the msg to everyone but the sender
                    let serialized = bincode::serialize(&ServerMsg::ChatMsg(msg))
                        .expect("could not serialize chat msg");

                    // TODO: be more efficient here
                    let addresses_to_send = {
                        let pool = tcp_writers_pool.lock().await;
                        let addresses = pool.keys().filter(|a| from.ne(a));
                        addresses.cloned().collect_vec()
                    };

                    debug!(
                        "sending chat message to {} players",
                        addresses_to_send.len()
                    );

                    let mut tcp_pool = tcp_writers_pool.lock().await;
                    for addr in addresses_to_send {
                        if let Some(writer) = tcp_pool.get_mut(&addr) {
                            if let Err(e) = writer.write_all(&serialized).await {
                                error!("failed to send chat message to {}: {:?}", addr, e);
                            }
                        }
                    }
                }
            }
        }

        anyhow::Ok(())
    });

    // not a fan of how this looks but it works ok.
    // it bubbles up to main on the first error and
    // exits the program with an error.
    tokio::try_join!(
        async { task1_handle.await? },
        async { task2_handle.await? },
        async { task3_handle.await? },
        async { task4_handle.await? },
    )?;

    Ok(())
}

/// Spins up a task to listen to incoming TCP messages
/// and relays them to the server channel.
fn handle_tcp_reader(
    mut tcp_read: OwnedReadHalf,
    server_channel_sender: UnboundedSender<ServerChannel>,
) {
    tokio::spawn(async move {
        let peer_addr = tcp_read.peer_addr()?;
        // make sure this is enough buffer size
        let mut buffer = [0; 1024];
        loop {
            match tcp_read.read(&mut buffer).await {
                Ok(0) | Err(_) => {
                    info!("{:?} closed TCP connnection or tcp read failed.", peer_addr);
                    let disconnect = ServerChannel::Disconnect(peer_addr);
                    server_channel_sender
                        .send(disconnect)
                        .context("could not send message to server channel")?;
                    break;
                }
                Ok(n) => {
                    let msg = bincode::deserialize::<ClientMsg>(&buffer[..n])
                        .context("could not deserialize msg from client. closing connection.")?;
                    server_channel_sender
                        .send(ServerChannel::from_client_msg(msg, tcp_read.peer_addr()?))
                        .context("could not send message to server channel")?;
                }
            }
        }

        anyhow::Ok(())
    });
}

fn validate_client(
    mut tcp_stream: TcpStream,
    tcp_writers_pool: Arc<Mutex<HashMap<SocketAddr, OwnedWriteHalf>>>,
    players: Arc<Mutex<HashMap<Uuid, Player>>>,
    server_channel_sender: UnboundedSender<ServerChannel>,
) {
    tokio::spawn(async move {
        let mut buf = [0; 1024];
        let size = tcp_stream.read(&mut buf).await?;
        let c_msg: ClientMsg = bincode::deserialize(&buf[..size])?;
        let ClientMsg::Init(username) = c_msg else {
            bail!("invalid client message");
        };

        println!("submitted username is: {}", username);

        let is_username_taken = (players.lock().await)
            .values()
            .any(|p| p.username == username);

        if is_username_taken {
            let str = format!("username: {} is taken.", username);

            info!("{}", str);

            let msg = ServerMsg::InitErr(str);
            let s_msg = bincode::serialize(&msg).unwrap();

            _ = tcp_stream.write(&s_msg).await;

            return anyhow::Ok(());
        }

        let player_id = Uuid::new_v4();
        let spawn_location = (0, 0);

        let player = Player {
            id: player_id,
            username,
            location: spawn_location,
            client_request_id: 0,
            udp_socket: None,
            tcp_socket: Some(tcp_stream.peer_addr()?),
        };

        (players.lock().await).insert(player_id, player);

        let init_ok = ServerMsg::InitOk(player_id, spawn_location);
        let s_init_ok = bincode::serialize(&init_ok)?;
        tcp_stream.write(&s_init_ok).await?;

        let (tcp_read, tcp_write) = tcp_stream.into_split();

        tcp_writers_pool
            .lock()
            .await
            .insert(tcp_read.peer_addr()?, tcp_write);

        handle_tcp_reader(tcp_read, server_channel_sender.clone());

        anyhow::Ok(())
    });
}
