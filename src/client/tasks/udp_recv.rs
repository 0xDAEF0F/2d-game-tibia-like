use crate::{
    UdpServerMsg,
    client::ClientChannel,
};
use anyhow::Result;
use std::sync::Arc;
use tokio::{net::UdpSocket, sync::mpsc::UnboundedSender, task::JoinHandle};

pub fn udp_recv_task(
    udp_socket: Arc<UdpSocket>,
    cc_tx: UnboundedSender<ClientChannel>,
) -> JoinHandle<Result<()>> {
    tokio::spawn(async move {
        let mut buf = [0; 1024];
        while let Ok(size) = udp_socket.recv(&mut buf).await {
            if let Ok(ps) = bincode::deserialize::<UdpServerMsg>(&buf[..size]) {
                match ps {
                    UdpServerMsg::PlayerMove {
                        location,
                        client_request_id,
                    } => todo!(),
                    UdpServerMsg::RestOfPlayers(other_players) => todo!(),
                    UdpServerMsg::Objects(game_objects) => todo!(),
                    UdpServerMsg::Pong(_) => todo!(),
                };
            }
        }
        Ok(())
    })
}
