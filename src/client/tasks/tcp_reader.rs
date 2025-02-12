use crate::TcpServerMsg;
use crate::client::{Cc, ClientChannel};
use anyhow::Result;
use log::debug;
use tokio::io::{AsyncReadExt, BufReader};
use tokio::net::tcp::OwnedReadHalf;
use tokio::sync::mpsc::UnboundedSender;
use tokio::task::JoinHandle;
use uuid::Uuid;

pub fn tcp_reader_task(
    tcp_read: OwnedReadHalf,
    cc_tx: UnboundedSender<ClientChannel>,
    user_id: Uuid,
) -> JoinHandle<Result<()>> {
    tokio::spawn(async move {
        let mut buf = [0; 1024];
        let mut reader = BufReader::new(tcp_read);

        while let Ok(size) = reader.read(&mut buf).await {
            debug!("received msg from server through the tcp reader");
            let server_msg: TcpServerMsg = bincode::deserialize(&buf[0..size])
                .expect("could not deserialize message from server in tcp listener");

            let cc = match server_msg {
                TcpServerMsg::Pong(ping_id) => Cc::Pong(ping_id),
                TcpServerMsg::ChatMsg { username, msg } => Cc::ChatMsg {
                    from: username,
                    msg,
                },
                TcpServerMsg::InitOk(_, _) => unreachable!(),
                TcpServerMsg::InitErr(_) => unreachable!(),
            };

            let msg = ClientChannel {
                id: user_id,
                msg: cc,
            };

            cc_tx.send(msg).unwrap();
        }
        Ok(())
    })
}
