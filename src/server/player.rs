use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Player {
    pub id: Uuid,
    pub username: String,
    pub client_request_id: u32,
    pub location: (u32, u32),

    pub tcp_socket: Option<SocketAddr>,
    pub udp_socket: Option<SocketAddr>,
}
