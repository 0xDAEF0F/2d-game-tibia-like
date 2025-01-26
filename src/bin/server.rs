use anyhow::Result;
use game_macroquad_example::{PlayerState, SERVER_ADDR};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::UdpSocket;
use tokio::sync::{Mutex, mpsc};
use tokio::time::{self, Duration};

const SERVER_TICK_RATE: u64 = 16; // ms

#[tokio::main]
async fn main() -> Result<()> {
    let socket = Arc::new(UdpSocket::bind(SERVER_ADDR).await?);
    let (tx, mut rx) = mpsc::unbounded_channel::<(SocketAddr, (usize, usize))>();

    println!("Server listening on: {}", SERVER_ADDR);

    let players = HashMap::<SocketAddr, (usize, usize)>::new();
    let players = Arc::new(Mutex::new(players));

    // game loop
    let players_clone = players.clone();
    let socket_send = socket.clone();
    tokio::spawn(async move {
        let mut interval = time::interval(Duration::from_millis(SERVER_TICK_RATE));
        loop {
            interval.tick().await;
            let players = players_clone.lock().await;

            for (&addr, _) in players.iter() {
                for (&a, &location) in players.iter() {
                    let player_state = PlayerState { id: a, location };
                    let serialized_ps = bincode::serialize(&player_state)?;
                    // what to do in case you can't send to the client?
                    _ = socket_send.send_to(&serialized_ps, addr).await;
                }
            }
        }
        #[allow(unreachable_code)]
        anyhow::Ok(())
    });

    // this task receives UDP packets
    let socket_recv = socket.clone();
    tokio::spawn(async move {
        let mut buf = [0; 1024];
        loop {
            if let Ok((size, src)) = socket_recv.recv_from(&mut buf).await {
                let maybe_ps = bincode::deserialize::<PlayerState>(&buf[..size]);
                if let Ok(ps) = maybe_ps {
                    println!("player state: {ps:?}");
                    _ = tx.send((src, ps.location));
                };
            }
        }
    });

    while let Some((socket_addr, location)) = rx.recv().await {
        let mut players = players.lock().await;
        players.insert(socket_addr, location);
    }

    Ok(())
}
