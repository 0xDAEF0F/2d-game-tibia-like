use anyhow::Result;
use game_macroquad_example::{Message, PlayerState, SERVER_TCP_ADDR, SERVER_UDP_ADDR};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::{TcpListener, UdpSocket};
use tokio::sync::{Mutex, mpsc};
use tokio::task::JoinHandle;
use tokio::time::{self, Duration};

const SERVER_TICK_RATE: u64 = 16; // ms

enum ServerChannel {
    PlayerState(PlayerState),
    Disconnect(SocketAddr),
}

#[tokio::main]
async fn main() -> Result<()> {
    let socket = Arc::new(UdpSocket::bind(SERVER_UDP_ADDR).await?);
    let _tcp_listener = TcpListener::bind(SERVER_TCP_ADDR).await?;

    println!("Server listening on UDP: {}", SERVER_UDP_ADDR);
    println!("Server listening on TCP: {}", SERVER_TCP_ADDR);

    let (tx, mut rx) = mpsc::unbounded_channel::<ServerChannel>();

    let players = HashMap::<SocketAddr, PlayerState>::new();
    let players = Arc::new(Mutex::new(players));

    // game loop
    let players_clone = players.clone();
    let socket_send = socket.clone();
    let _: JoinHandle<Result<()>> = tokio::spawn(async move {
        let mut interval = time::interval(Duration::from_millis(SERVER_TICK_RATE));
        loop {
            interval.tick().await;
            let players = players_clone.lock().await;

            for addr in players.keys().cloned() {
                let ps = players.get(&addr).cloned().unwrap();
                let ps_ser = bincode::serialize(&Message::PlayerState(ps))?;
                _ = socket_send.send_to(&ps_ser, addr).await;

                let rest = players
                    .values()
                    .filter(|ps| ps.id != addr)
                    .map(|ps| PlayerState {
                        id: ps.id,
                        client_request_id: None,
                        location: ps.location,
                    });

                let rest_players = Message::RestOfPlayers(rest.collect());
                let rest_players_ser = bincode::serialize(&rest_players)?;
                _ = socket_send.send_to(&rest_players_ser, addr).await;
            }
        }
    });

    // receives msgs from clients
    let socket_recv = socket.clone();
    tokio::spawn(async move {
        let mut buf = [0; 1024];
        loop {
            if let Ok((size, src)) = socket_recv.recv_from(&mut buf).await {
                let maybe_ps = bincode::deserialize::<Message>(&buf[..size]);
                if let Ok(msg) = maybe_ps {
                    let sc = match msg {
                        Message::Disconnect => ServerChannel::Disconnect(src),
                        Message::PlayerState(ps) => ServerChannel::PlayerState(ps),
                        Message::RestOfPlayers(_) => unreachable!(),
                    };
                    _ = tx.send(sc);
                } else {
                    eprintln!("failed to deserialize message from {src:?}");
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

                // still needs some form of validation to check if the location is valid
                player.client_request_id = ps.client_request_id;
                player.location = ps.location;
            }
            ServerChannel::Disconnect(addr) => {
                println!("{addr:?} disconnected");
                players.lock().await.remove(&addr);
            }
        }
    }

    Ok(())
}
