use anyhow::Result;
use serde::Serialize;
use std::net::SocketAddr;
use tokio::net::UdpSocket;

pub trait SendableSync {
    fn send_msg<T: Serialize>(&self, msg: &T, to: Option<SocketAddr>) -> Result<usize>;
    fn send_msg_default<T: Serialize>(&self, msg: &T) -> Result<usize>;
}

impl SendableSync for UdpSocket {
    fn send_msg<T: Serialize>(&self, msg: &T, to: Option<SocketAddr>) -> Result<usize> {
        let buf = bincode::serialize(msg)?;
        if self.peer_addr().is_ok() {
            Ok(self.try_send(&buf)?)
        } else if let Some(addr) = to {
            Ok(self.try_send_to(&buf, addr)?)
        } else {
            Err(anyhow::anyhow!(
                "Socket is not connected and no destination address provided"
            ))
        }
    }

    fn send_msg_default<T: Serialize>(&self, msg: &T) -> Result<usize> {
        self.send_msg(msg, None)
    }
}
