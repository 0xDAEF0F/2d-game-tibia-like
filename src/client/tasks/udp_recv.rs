use crate::{
	OtherPlayer,
	client::{Cc, ClientChannel},
	udp::UdpServerMsg,
};
use anyhow::Result;
use std::sync::Arc;
use tokio::{net::UdpSocket, sync::mpsc::UnboundedSender, task::JoinHandle};
use uuid::Uuid;

pub fn udp_recv_task(
	udp_socket: Arc<UdpSocket>,
	cc_tx: UnboundedSender<ClientChannel>,
	user_id: Uuid,
) -> JoinHandle<Result<()>> {
	tokio::spawn(async move {
		let mut buf = [0; 1024];
		while let Ok(size) = udp_socket.recv(&mut buf).await {
			if let Ok(ps) = bincode::deserialize::<UdpServerMsg>(&buf[..size]) {
				match ps {
					UdpServerMsg::PlayerMove {
						location,
						client_request_id,
					} => {
						let cc = ClientChannel {
							id: user_id,
							msg: crate::client::Cc::PlayerMove {
								client_request_id,
								location,
							},
						};
						cc_tx.send(cc)?;
					}
					UdpServerMsg::Objects(game_objects) => {
						let cc = ClientChannel {
							id: user_id,
							msg: Cc::Objects(game_objects),
						};
						cc_tx.send(cc)?;
					}
					UdpServerMsg::Pong(_) => todo!(),
					UdpServerMsg::OtherPlayer {
						username,
						location,
						direction,
					} => {
						let cc = ClientChannel {
							id: user_id,
							msg: Cc::OtherPlayer(OtherPlayer {
								username,
								location,
								direction,
							}),
						};
						cc_tx.send(cc)?;
					}
					UdpServerMsg::PlayerHealthUpdate { hp } => {
						let cc = ClientChannel {
							id: user_id,
							msg: Cc::PlayerHealthUpdate { hp },
						};
						cc_tx.send(cc)?;
					}
				};
			}
		}
		Ok(())
	})
}
