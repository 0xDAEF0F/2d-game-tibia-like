use game_macroquad_example::{PlayerState, SERVER_ADDR};
use renet::{ConnectionConfig, DefaultChannel, RenetServer, ServerEvent};
use renet_netcode::{NetcodeServerTransport, ServerAuthentication, ServerConfig};
use std::collections::HashMap;
use std::net::UdpSocket;
use std::time::SystemTime;
use tokio::time::Duration;

fn main() {
    let mut server = RenetServer::new(ConnectionConfig::default());

    let socket: UdpSocket = UdpSocket::bind(SERVER_ADDR).unwrap();
    let server_config = ServerConfig {
        current_time: SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap(),
        max_clients: 64,
        protocol_id: 0,
        public_addresses: vec![SERVER_ADDR],
        authentication: ServerAuthentication::Unsecure,
    };
    let mut transport = NetcodeServerTransport::new(server_config, socket).unwrap();

    println!("Server running on {}", SERVER_ADDR);

    let mut _players_position: HashMap<usize, (usize, usize)> = HashMap::new();

    loop {
        let delta_time = Duration::from_millis(16);
        server.update(delta_time);
        transport.update(delta_time, &mut server).unwrap();

        while let Some(event) = server.get_event() {
            match event {
                ServerEvent::ClientConnected { client_id } => {
                    println!("Client {client_id} connected");
                }
                ServerEvent::ClientDisconnected { client_id, reason } => {
                    println!("Client {client_id} disconnected: {reason}");
                }
            }
        }

        for client_id in server.clients_id() {
            while let Some(message) =
                server.receive_message(client_id, DefaultChannel::ReliableOrdered)
            {
                let player_state: PlayerState = bincode::deserialize(&message).unwrap();

                let player_state_bytes: Vec<u8> = bincode::serialize(&player_state).unwrap();

                println!(
                    "sending message to client {client_id}: {:?}",
                    player_state.location
                );

                server.send_message(
                    client_id,
                    DefaultChannel::ReliableOrdered,
                    player_state_bytes,
                );
            }
        }

        transport.send_packets(&mut server);

        std::thread::sleep(delta_time); // Running at 60hz
    }
}
