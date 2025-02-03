use anyhow::Result;
use log::{debug, error, info};
use my_mmo::*;
use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::tcp::OwnedWriteHalf;
use tokio::net::{TcpListener, TcpStream, UdpSocket};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tokio::sync::{Mutex, mpsc};
use tokio::time::{self, Duration};

const SERVER_TICK_RATE: u64 = 16; // ms

type Location = (usize, usize);

enum ServerChannel {
    PlayerState(PlayerState),
    Disconnect(SocketAddr),
    MoveObject { from: Location, to: Location },
    Msg { from: SocketAddr, msg: String },
}

#[tokio::main]
async fn main() -> Result<()> {
    MmoLogger::init("debug");

    let socket = Arc::new(UdpSocket::bind(SERVER_UDP_ADDR).await?);
    let tcp_listener = TcpListener::bind(SERVER_TCP_ADDR).await?;

    info!("Server listening on UDP: {}", SERVER_UDP_ADDR);
    info!("Server listening on TCP: {}", SERVER_TCP_ADDR);

    let (tx, mut rx) = mpsc::unbounded_channel::<ServerChannel>();

    let players = HashMap::<SocketAddr, PlayerState>::new();
    let players = Arc::new(Mutex::new(players));

    let game_objects = create_game_objects();
    let game_objects = Arc::new(Mutex::new(game_objects));

    let (tx_, rx_) = mpsc::unbounded_channel();

    let connection_writers: HashMap<SocketAddr, OwnedWriteHalf> = HashMap::new();
    let connection_writers = Arc::new(Mutex::new(connection_writers));

    // accept tcp connections
    tokio::spawn(async move {
        while let Ok((socket, addr)) = tcp_listener.accept().await {
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
                                        let sc = ServerChannel::Msg {
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
        tx.clone(),
        connection_writers.clone(),
    ));

    // game loop
    let players_clone = players.clone();
    let objects_clone = game_objects.clone();
    let socket_send = socket.clone();
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
                _ = socket_send.send_to(&ps_ser, addr).await;

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
                _ = socket_send.send_to(&rest_players_ser, addr).await;

                let objects = ServerMsg::Objects(game_objects.clone());
                let encoded_objects =
                    bincode::serialize(&objects).expect("could not serialize game objects");
                _ = socket_send.send_to(&encoded_objects, addr).await;
            }
        }
    });

    // receives msgs from clients
    let socket_recv = socket.clone();
    tokio::spawn(async move {
        let mut buf = [0; 1024];
        loop {
            if let Ok((size, src)) = socket_recv.recv_from(&mut buf).await {
                let maybe_ps = bincode::deserialize::<ClientMsg>(&buf[..size]);
                if let Ok(msg) = maybe_ps {
                    if let ClientMsg::Ping(x) = &msg {
                        let pong = bincode::serialize(&ServerMsg::Pong(*x)).unwrap();
                        let _result = socket_recv.try_send_to(&pong, src);
                        debug!("sending pong for {:?}", *x);
                        continue;
                    }

                    let sc = match msg {
                        ClientMsg::Disconnect => ServerChannel::Disconnect(src),
                        ClientMsg::PlayerState(ps) => ServerChannel::PlayerState(ps),
                        ClientMsg::MoveObject { from, to } => {
                            ServerChannel::MoveObject { from, to }
                        }
                        ClientMsg::ChatMsg(_) => todo!(),
                        ClientMsg::Ping(_) => unreachable!(),
                    };
                    _ = tx.send(sc);
                } else {
                    debug!("failed to deserialize message from {src:?}");
                };
            }
        }
    });

    while let Some(ps) = rx.recv().await {
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
                connection_writers.lock().await.remove(&addr);
            }
            ServerChannel::Msg { from, msg } => {
                // relay the msg to everyone but the sender
                let serialized = bincode::serialize(&ServerMsg::ChatMsg(msg))
                    .expect("could not serialize chat msg");

                let players = players.lock().await;
                let players = players.keys().filter(|a| from.ne(a));

                for addr in players {
                    if let Some(writer) = connection_writers.lock().await.get_mut(addr) {
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
