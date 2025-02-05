use anyhow::Result;
use itertools::Itertools;
use log::{debug, error, info, trace};
use my_mmo::*;
use std::collections::{HashMap, hash_map::Entry};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::tcp::OwnedWriteHalf;
use tokio::net::{TcpListener, TcpStream, UdpSocket};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tokio::sync::{Mutex, mpsc};
use tokio::time::{self, Duration};

const SERVER_TICK_RATE: u64 = 16; // how often the server loops. ms.

enum ServerChannel {
    PlayerState(PlayerState),
    Disconnect(SocketAddr),
    MoveObject { from: Location, to: Location },
    ChatMsg { from: SocketAddr, msg: String },
}

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

    let (tx_, rx_) = mpsc::unbounded_channel::<(TcpStream, SocketAddr)>();

    let tcp_writers_pool: HashMap<SocketAddr, OwnedWriteHalf> = HashMap::new();
    let tcp_writers_pool = Arc::new(Mutex::new(tcp_writers_pool));

    // Accepts TCP connections. Depends on `tcp_listener` and `tx_`.
    tokio::spawn(async move {
        while let Ok((socket, addr)) = tcp_listener.accept().await {
            info!("accepted TCP connection from: {}", addr);
            if let Err(e) = tx_.send((socket, addr)) {
                error!("could not send the tcp stream to the receiver: {}", e);
            }
        }
    });

    async fn process_connection(
        mut rx: UnboundedReceiver<(TcpStream, SocketAddr)>,
        sender_channel: UnboundedSender<ServerChannel>,
        tcp_write_pool: Arc<Mutex<HashMap<SocketAddr, OwnedWriteHalf>>>,
    ) {
        while let Some((socket, addr)) = rx.recv().await {
            let cloned_server_channel = sender_channel.clone();
            let (mut tcp_read, tcp_write) = socket.into_split();
            tcp_write_pool.lock().await.insert(addr, tcp_write);
            tokio::spawn(async move {
                let mut buffer = [0; 1024];
                loop {
                    match tcp_read.read(&mut buffer).await {
                        Ok(0) => {
                            info!("{} closed TCP connnection", addr);
                            break;
                        }
                        Ok(n) => {
                            if let Ok(msg) = bincode::deserialize::<ClientMsg>(&buffer[..n]) {
                                match msg {
                                    ClientMsg::ChatMsg(msg) => {
                                        let sc = ServerChannel::ChatMsg {
                                            from: addr,
                                            msg: msg.to_string(),
                                        };
                                        if cloned_server_channel.send(sc).is_err() {
                                            error!("could not send msg internally?");
                                        }
                                    }
                                    _ => todo!(),
                                }
                            } else {
                                error!("could not deserialize message from {:?}", addr);
                            }
                        }
                        Err(e) => {
                            eprintln!("failed to read from socket; err = {:?}", e);
                            break;
                        }
                    }
                }
            });
        }
    }

    tokio::spawn(process_connection(
        rx_,
        server_channel_sender.clone(),
        tcp_writers_pool.clone(),
    ));

    // game loop. depends on `rx`, `players`, `game_objects`, and `udp_socket`.
    let players_clone = players.clone();
    let objects_clone = game_objects.clone();
    let udp_socket_ = udp_socket.clone();
    tokio::spawn(async move {
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
    tokio::spawn(async move {
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
                tcp_writers_pool.lock().await.remove(&addr);
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

    Ok(())
}
