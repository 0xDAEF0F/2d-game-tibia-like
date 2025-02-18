use crate::TcpServerMsg;
use crate::client::{Cc, ClientChannel};
use anyhow::Result;
use log::{debug, info};
use tokio::io::AsyncReadExt;
use tokio::net::tcp::OwnedReadHalf;
use tokio::sync::mpsc::UnboundedSender;
use tokio::task::JoinHandle;
use uuid::Uuid;

pub fn tcp_reader_task(
    mut tcp_read: OwnedReadHalf,
    cc_tx: UnboundedSender<ClientChannel>,
    user_id: Uuid,
) -> JoinHandle<Result<()>> {
    tokio::spawn(async move {
        let mut buf = [0; 1024];
        loop {
            match tcp_read.read(&mut buf).await {
                Ok(size) if size > 0 => {
                    debug!("received msg from server through the tcp reader");

                    let Ok(server_msg) = bincode::deserialize::<TcpServerMsg>(&buf[0..size]) else {
                        debug!("could not deserialize message from server in tcp listener");
                        continue;
                    };

                    let cc = match server_msg {
                        TcpServerMsg::Pong(ping_id) => Cc::Pong(ping_id),
                        TcpServerMsg::ChatMsg { username, msg } => Cc::ChatMsg {
                            from: username,
                            msg,
                        },
                        TcpServerMsg::ReconnectOk => Cc::ReconnectOk,
                        TcpServerMsg::InitOk(_) => unreachable!(),
                        TcpServerMsg::InitErr(_) => unreachable!(),
                    };

                    let msg = ClientChannel {
                        id: user_id,
                        msg: cc,
                    };

                    cc_tx.send(msg).unwrap();
                }
                _ => {
                    info!("exiting tcp reader task.");
                    break;
                }
            }
        }
        Ok(())
    })
}
