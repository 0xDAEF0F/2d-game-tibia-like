use anyhow::{Context, Result};
use game_macroquad_example::{PlayerState, SERVER_ADDR};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::UdpSocket;
use tokio::sync::{Mutex, mpsc};
use tokio::task::JoinHandle;
use tokio::time::{self, Duration};

const SERVER_TICK_RATE: u64 = 16; // ms

#[tokio::main]
async fn main() -> Result<()> {
    let socket = Arc::new(UdpSocket::bind(SERVER_ADDR).await?);
    let (tx, mut rx) = mpsc::unbounded_channel::<PlayerState>();

    println!("Server listening on: {}", SERVER_ADDR);

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

            for (&addr, _) in players.iter() {
                for (&a, ps) in players.iter() {
                    let player_state = PlayerState {
                        id: a,
                        client_request_id: ps.client_request_id,
                        location: ps.location,
                    };
                    let serialized_ps = bincode::serialize(&player_state)?;
                    // what to do in case you can't send to the client?
                    _ = socket_send.send_to(&serialized_ps, addr).await;
                }
            }
        }
    });

    // receives msgs from clients
    let socket_recv = socket.clone();
    tokio::spawn(async move {
        let mut buf = [0; 1024];
        loop {
            if let Ok((size, src)) = socket_recv.recv_from(&mut buf).await {
                let maybe_ps = bincode::deserialize::<PlayerState>(&buf[..size]);
                if let Ok(ps) = maybe_ps {
                    _ = tx.send(ps);
                } else {
                    eprintln!("server: failed to deserialize `PlayerState`");
                };
            }
        }
    });

    while let Some(ps) = rx.recv().await {
        let mut players = players.lock().await;

        if !players.contains_key(&ps.id) {
            players.insert(ps.id, ps);
            continue;
        }

        let player = players.get_mut(&ps.id).unwrap();

        if ps.client_request_id < player.client_request_id {
            continue;
        }

        // still needs some form of validation to check if the location is valid
        player.client_request_id = ps.client_request_id;
        player.location = ps.location;
    }

    Ok(())
}
