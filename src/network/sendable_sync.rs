use anyhow::Result;
use serde::Serialize;
use std::net::SocketAddr;
use tokio::net::UdpSocket;

pub trait SendableSync {
    fn send_msg<T: Serialize>(&self, msg: &T, to: Option<SocketAddr>) -> Result<usize>;
    fn send_msg_and_log<T: Serialize>(&self, msg: &T, to: Option<SocketAddr>);
}

impl SendableSync for UdpSocket {
    fn send_msg<T: Serialize>(&self, msg: &T, to: Option<SocketAddr>) -> Result<usize> {
        let buf = bincode::serialize(msg)?;

        let bytes_sent = match to {
            None => self.try_send(&buf),
            Some(addr) => self.try_send_to(&buf, addr),
        }?;

        Ok(bytes_sent)
    }

    fn send_msg_and_log<T: Serialize>(&self, msg: &T, to: Option<SocketAddr>) {
        let result = self.send_msg(msg, to);
        match result {
            Ok(bytes_sent) => {
                log::trace!("Sent {bytes_sent} through UDP.",);
            }
            Err(err) => {
                log::error!("Failed to send UDP message: {:?}", err);
            }
        }
    }
}
