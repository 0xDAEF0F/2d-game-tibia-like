use anyhow::Result;
use async_trait::async_trait;
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

#[async_trait]
pub trait SendableAsync {
   async fn send_msg_<T>(&self, msg: T, to: Option<SocketAddr>) -> Result<usize>
   where
      T: Serialize + Send;

   async fn send_msg_and_log_<T>(&self, msg: T, to: Option<SocketAddr>)
   where
      T: Serialize + Send;
}

#[async_trait]
impl SendableAsync for UdpSocket {
   async fn send_msg_<T>(&self, msg: T, to: Option<SocketAddr>) -> Result<usize>
   where
      T: Serialize + Send,
   {
      let data = bincode::serialize(&msg)?;

      let sent_bytes = match to {
         Some(addr) => self.send_to(&data, addr).await?,
         None => self.send(&data).await?,
      };

      Ok(sent_bytes)
   }

   async fn send_msg_and_log_<T>(&self, msg: T, to: Option<SocketAddr>)
   where
      T: Serialize + Send,
   {
      match self.send_msg_(msg, to).await {
         Ok(size) => {
            log::trace!("Sent {size} bytes");
         }
         Err(e) => {
            log::error!("Failed to send UDP message: {:?}", e);
         }
      }
   }
}
