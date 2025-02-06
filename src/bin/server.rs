use anyhow::{Context, Result, anyhow, bail};
use itertools::Itertools;
use log::{debug, error, info, trace};
use my_mmo::server::ServerChannel;
use my_mmo::*;
use std::collections::{HashMap, hash_map::Entry};
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::Mutex as SMutex;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::{TcpListener, TcpStream, UdpSocket};
use tokio::sync::mpsc::UnboundedSender;
use tokio::sync::{Mutex, mpsc};
use tokio::time::{self, Duration};
use tokio_stream::StreamExt;

const SERVER_TICK_RATE: u64 = 16; // how often the server loops. ms.

// TODO: the server is supposed to map tcp/udp clients from their socket
// addresses to their player_id/username.

#[tokio::main]
async fn main() -> Result<()> {
    MmoLogger::init("debug");

    let udp_socket = Arc::new(UdpSocket::bind(SERVER_UDP_ADDR).await?);
    let tcp_listener = TcpListener::bind(SERVER_TCP_ADDR).await?;

    info!("Server listening on UDP: {}", SERVER_UDP_ADDR);
    info!("Server listening on TCP: {}", SERVER_TCP_ADDR);

    let (server_channel_sender, mut server_channel_receiver) =
        mpsc::unbounded_channel::<ServerChannel>();

    let players = HashMap::<SocketAddr, PlayerState>::new();
    let players = Arc::new(Mutex::new(players));

    let game_objects = GameObjects::new();
    let game_objects = Arc::new(Mutex::new(game_objects));

    let tcp_writers_pool: HashMap<SocketAddr, OwnedWriteHalf> = HashMap::new();
    let tcp_writers_pool = Arc::new(SMutex::new(tcp_writers_pool));

    // Accepts TCP connections. Depends on `tcp_listener` and `tx_`.
    let tcp_writers_pool_clone = Arc::clone(&tcp_writers_pool);
    let players_clone = Arc::clone(&players);
    let server_channel_sender_clone = server_channel_sender.clone();
    let _task1_handle = tokio::spawn(async move {
        use tokio_stream::wrappers::TcpListenerStream;
        let mut iter = TcpListenerStream::new(tcp_listener);

        while let Ok(tcp_stream) = iter.next().await.context("tcp stream ended")? {
            let peer_addr = tcp_stream
                .peer_addr()
                .context("failed resolve `peer_addr()` of `TcpStream`")?;

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
    let _task2_handle = tokio::spawn(async move {
        let mut interval = time::interval(Duration::from_millis(SERVER_TICK_RATE));
        loop {
            interval.tick().await;
            let players = players_clone.lock().await;
            let game_objects = objects_clone.lock().await;

            for addr in players.keys().cloned() {
                let ps = players.get(&addr).cloned().unwrap();
                let ps_ser = bincode::serialize(&ServerMsg::PlayerState(ps))
                    .expect("could not serialize `PlayerState`");
                _ = udp_socket_.send_to(&ps_ser, addr).await;

                let rest = players
                    .values()
                    .filter(|ps| ps.id != addr)
                    .map(|ps| PlayerState {
                        id: ps.id,
                        client_request_id: None,
                        location: ps.location,
                    });

                let rest_players = ServerMsg::RestOfPlayers(rest.collect());
                let rest_players_ser =
                    bincode::serialize(&rest_players).expect("could not serialize `RestPlayers`");
                _ = udp_socket_.send_to(&rest_players_ser, addr).await;

                let objects = ServerMsg::Objects(game_objects.clone());
                let encoded_objects =
                    bincode::serialize(&objects).expect("could not serialize game objects");
                _ = udp_socket_.send_to(&encoded_objects, addr).await;
            }
        }
    });

    // Receives UDP msgs from clients. depends on the UDP socket and server channel sender.
    let udp_socket_ = udp_socket.clone();
    let _task3_handle = tokio::spawn(async move {
        let mut buf = [0; 1024];
        while let Ok((size, src)) = udp_socket_.recv_from(&mut buf).await {
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
                ClientMsg::PlayerState(ps) => ServerChannel::PlayerState(ps),
                ClientMsg::MoveObject { from, to } => ServerChannel::MoveObject { from, to },
                ClientMsg::ChatMsg(_) => unimplemented!("chat messages only allowed through TCP"),
                ClientMsg::Ping(_) => unreachable!(),
                ClientMsg::Init(_) => unreachable!(),
            };

            _ = server_channel_sender.send(sc);
        }
    });

    // Receives messages from the server channel sender. depends on `rx`.
    while let Some(ps) = server_channel_receiver.recv().await {
        match ps {
            ServerChannel::PlayerState(ps) => {
                let mut players = players.lock().await;

                if let Entry::Vacant(e) = players.entry(ps.id) {
                    e.insert(ps);
                    continue;
                }

                let player = players.get_mut(&ps.id).unwrap();

                if ps.client_request_id <= player.client_request_id {
                    continue;
                }

                if ps.location == player.location {
                    continue;
                }

                debug!(
                    "received player state from {:?} at {:?}",
                    ps.id, ps.location
                );
                // still needs some form of validation to check if the location is valid
                player.client_request_id = ps.client_request_id;
                player.location = ps.location;
            }
            ServerChannel::MoveObject { from, to } => {
                debug!("received move object from {:?} to {:?}", from, to);
                let mut game_objects = game_objects.lock().await;
                if let Some(obj) = game_objects.0.remove(&from) {
                    game_objects.0.insert(to, obj);
                }
            }
            ServerChannel::Disconnect(addr) => {
                info!("{addr:?} disconnected");
                players.lock().await.remove(&addr);
                tcp_writers_pool.lock().unwrap().remove(&addr);
            }
            ServerChannel::ChatMsg { from, msg } => {
                debug!("received chat msg: \"{}\" from: {}", msg, from);

                // relay the msg to everyone but the sender
                let serialized = bincode::serialize(&ServerMsg::ChatMsg(msg))
                    .expect("could not serialize chat msg");

                // TODO: be more efficient here
                let addresses_to_send = {
                    let pool = tcp_writers_pool.lock().unwrap();
                    let addresses = pool.keys().filter(|a| from.ne(a));
                    addresses.cloned().collect_vec()
                };

                debug!(
                    "sending chat message to {} players",
                    addresses_to_send.len()
                );

                let mut tcp_pool = tcp_writers_pool.lock().unwrap();
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

    Ok(())
}

/// Spins up a task to listen to incoming TCP messages
/// and relays them to the server channel.
fn handle_tcp_reader(
    mut tcp_read: OwnedReadHalf,
    server_channel_sender: UnboundedSender<ServerChannel>,
) {
    tokio::spawn(async move {
        // TODO: make sure this is enough buffer size
        let mut buffer = [0; 1024];
        loop {
            match tcp_read.read(&mut buffer).await {
                // TODO: cleanup
                Ok(0) => {
                    info!("{:?} closed TCP connnection", tcp_read.peer_addr());
                    break;
                }
                Ok(n) => {
                    let msg = bincode::deserialize::<ClientMsg>(&buffer[..n])
                        .context("could not deserialize msg from client. closing connection.")?;
                    server_channel_sender
                        .send(ServerChannel::from_client_msg(msg, tcp_read.peer_addr()?))
                        .context("could not send message to server channel")?;
                }
                Err(e) => {
                    eprintln!("failed to read from socket; err = {:?}", e);
                    break;
                }
            }
        }

        anyhow::Ok(())
    });
}

fn validate_client(
    mut tcp_stream: TcpStream,
    tcp_writers_pool: Arc<SMutex<HashMap<SocketAddr, OwnedWriteHalf>>>,
    players: Arc<Mutex<HashMap<SocketAddr, PlayerState>>>,
    server_channel_sender: UnboundedSender<ServerChannel>,
) {
    // discard errors for now
    _ = tokio::spawn(async move {
        let mut buf = [0; 1024];
        let size = tcp_stream.read(&mut buf).await?;
        let c_msg: ClientMsg = bincode::deserialize(&buf[..size])?;
        let ClientMsg::Init(_username) = c_msg else {
            bail!("invalid client message");
        };

        println!("submitted username is: {}", _username);

        // TODO: check if the username is available/valid in the players store
        _ = players.lock().await;

        let init_ok = ServerMsg::InitOk(42, (0, 0));
        let s_init_ok = bincode::serialize(&init_ok)?;
        let _bytes_sent = tcp_stream.write(&s_init_ok).await?;

        let (tcp_read, tcp_write) = tcp_stream.into_split();

        tcp_writers_pool
            .lock()
            .map_err(|_| anyhow!("failed to acquire lock on tcp_writers_pool"))?
            .insert(tcp_read.peer_addr()?, tcp_write);

        handle_tcp_reader(tcp_read, server_channel_sender.clone());

        anyhow::Ok(())
    });
}
