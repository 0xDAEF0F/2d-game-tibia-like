use anyhow::Result;
use log::info;
use my_mmo::constants::*;
use my_mmo::server::Player;
use my_mmo::server::ServerChannel;
use my_mmo::server::tasks::game_loop_task;
use my_mmo::server::tasks::sc_rx_task;
use my_mmo::server::tasks::tcp_listener_task;
use my_mmo::server::tasks::udp_recv_task;
use my_mmo::*;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::{TcpListener, UdpSocket};
use tokio::sync::{Mutex, mpsc};
use uuid::Uuid;

// TODO: the server is supposed to map tcp/udp clients from their socket
// addresses to their player_id/username.

// 1. Client establishes connection with server through TCP
//    and server sends `user_id` to the client
// 2. When client sends his UDP datagram he also sends his unique `user_id`
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

    // state
    let address_mapping: HashMap<SocketAddr, Uuid> = HashMap::new(); // tcp or udp addr -> player_id
    let address_mapping = Arc::new(Mutex::new(address_mapping));

    let players = HashMap::<Uuid, Player>::new();
    let players = Arc::new(Mutex::new(players));

    let game_objects = GameObjects::new();
    let game_objects = Arc::new(Mutex::new(game_objects));

    let (sc_tx, sc_rx) = mpsc::unbounded_channel::<ServerChannel>();

    let task1_handle = tcp_listener_task(
        tcp_listener,
        players.clone(),
        address_mapping.clone(),
        sc_tx.clone(),
    );

    // Game loop task
    let task2_handle = game_loop_task(udp_socket.clone(), players.clone(), game_objects.clone());

    // Receives UDP msgs from clients.
    let task3_handle = udp_recv_task(
        udp_socket.clone(),
        sc_tx.clone(),
        address_mapping.clone(),
        players.clone(),
    );

    // Handler/processor of server channel messages
    let task4_handle = sc_rx_task(sc_rx, udp_socket, address_mapping, players, game_objects);

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
