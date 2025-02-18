use crate::Location;
use std::net::SocketAddr;
use tokio::net::tcp::OwnedWriteHalf;
use uuid::Uuid;

#[derive(Debug)]
pub struct Player {
    pub id: Uuid,
    pub username: String,
    pub client_request_id: u32,
    pub location: Location,

    pub hp: u32,
    pub max_hp: u32,
    pub level: u32,

    pub tcp_tx: OwnedWriteHalf,
    pub tcp_socket: SocketAddr,
    pub udp_socket: Option<SocketAddr>,
}

impl Player {
    pub fn new(
        id: Uuid,
        username: String,
        tcp_socket: SocketAddr,
        tcp_tx: OwnedWriteHalf,
    ) -> Player {
        Player {
            id,
            username,
            client_request_id: 0,
            location: (0, 0),
            tcp_socket,
            udp_socket: None,
            tcp_tx,
            hp: 100,
            max_hp: 100,
            level: 1,
        }
    }
}
