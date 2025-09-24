use crate::{Player, Sc, ServerChannel};
use anyhow::Result;
use shared::network::udp::UdpClientMsg;
use std::{collections::HashMap, net::SocketAddr, sync::Arc};
use thin_logger::log::{debug, error};
use tokio::{
   net::UdpSocket,
   sync::{Mutex, mpsc::UnboundedSender},
   task::JoinHandle,
};
use uuid::Uuid;

pub fn udp_recv_task(
   udp_socket: Arc<UdpSocket>,
   sc_tx: UnboundedSender<ServerChannel>,
   _address_mapping: Arc<Mutex<HashMap<SocketAddr, Uuid>>>,
   players: Arc<Mutex<HashMap<Uuid, Player>>>,
) -> JoinHandle<Result<()>> {
   tokio::spawn(async move {
      let mut buf = [0; 1024];
      while let Ok((size, src)) = udp_socket.recv_from(&mut buf).await {
         let Ok(msg) = bincode::deserialize::<UdpClientMsg>(&buf[..size]) else {
            debug!("failed to deserialize UDP message from: {src}");
            continue;
         };

         let user_id = msg.get_player_id();

         if let Some(p) = (players.lock().await).get_mut(&user_id) {
            if p.udp_socket.is_none() {
               p.udp_socket = Some(src);
            }
         } else {
            error!("received UDP message that had a UUID that was not in the players list!");
            continue;
         };

         let msg = match msg {
            UdpClientMsg::Ping {
               client_request_id, ..
            } => Sc::Ping(client_request_id),
            UdpClientMsg::PlayerMove {
               client_request_id,
               location,
               ..
            } => Sc::PlayerMove {
               client_request_id,
               location,
            },
            UdpClientMsg::MoveObject { from, to, .. } => Sc::MoveObject { from, to },
         };

         let sc = ServerChannel { id: user_id, msg };

         if let Err(e) = sc_tx.send(sc) {
            error!("failed to send message from UDP recv to `sc_rx`: {e}");
         };
      }
      Ok(())
   })
}
