use anyhow::Result;
use env_logger::Env;
use game_macroquad_example::*;
use log::{debug, info};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::{TcpListener, UdpSocket};
use tokio::sync::{Mutex, mpsc};
use tokio::task::JoinHandle;
use tokio::time::{self, Duration};

const SERVER_TICK_RATE: u64 = 16; // ms

type Location = (usize, usize);

enum ServerChannel {
    PlayerState(PlayerState),
    Disconnect(SocketAddr),
    MoveObject { from: Location, to: Location },
}

#[tokio::main]
async fn main() -> Result<()> {
    let env = Env::default().default_filter_or("debug");
    env_logger::init_from_env(env);

    let socket = Arc::new(UdpSocket::bind(SERVER_UDP_ADDR).await?);
    let _tcp_listener = TcpListener::bind(SERVER_TCP_ADDR).await?;

    info!("Server listening on UDP: {}", SERVER_UDP_ADDR);
    info!("Server listening on TCP: {}", SERVER_TCP_ADDR);

    let (tx, mut rx) = mpsc::unbounded_channel::<ServerChannel>();

    let players = HashMap::<SocketAddr, PlayerState>::new();
    let players = Arc::new(Mutex::new(players));

    let game_objects = create_game_objects();
    let game_objects = Arc::new(Mutex::new(game_objects));

    // game loop
    let players_clone = players.clone();
    let objects_clone = game_objects.clone();
    let socket_send = socket.clone();
    let _: JoinHandle<Result<()>> = tokio::spawn(async move {
        let mut interval = time::interval(Duration::from_millis(SERVER_TICK_RATE));
        loop {
            interval.tick().await;
            let players = players_clone.lock().await;
            let game_objects = objects_clone.lock().await;

            for addr in players.keys().cloned() {
                let ps = players.get(&addr).cloned().unwrap();
                let ps_ser = bincode::serialize(&ServerMsg::PlayerState(ps))?;
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
                let rest_players_ser = bincode::serialize(&rest_players)?;
                _ = socket_send.send_to(&rest_players_ser, addr).await;

                let objects = ServerMsg::Objects(game_objects.clone());
                let encoded_objects = bincode::serialize(&objects)?;
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

                if !players.contains_key(&ps.id) {
                    players.insert(ps.id, ps);
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
            }
        }
    }

    Ok(())
}
