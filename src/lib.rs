use serde::{Deserialize, Serialize};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};

pub const SERVER_ADDR: SocketAddr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 5000);

#[derive(Debug, Serialize, Deserialize)]
pub enum Message {
    PlayerState(PlayerState),
    Disconnect,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PlayerState {
    pub id: SocketAddr,           // TODO: use another identifier
    pub client_request_id: u64,   // TODO: use another identifier
    pub location: (usize, usize), // (x, y)
}
