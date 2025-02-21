use crate::{InitPlayer, Location};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// Client -> Server
#[derive(Debug, Serialize, Deserialize)]
pub enum TcpClientMsg {
    PlayerState {
        id: Uuid,
        location: Location,
        client_request_id: u32,
    },
    MoveObject {
        from: Location,
        to: Location,
    },
    Disconnect,
    Ping(u32),
    ChatMsg(String),
    Init(String),
    Reconnect(Uuid),
}

#[derive(Debug, Serialize, Deserialize)]
pub enum TcpServerMsg {
    Pong(u32),
    ChatMsg { username: String, msg: String },
    InitOk(InitPlayer),
    ReconnectOk,
    InitErr(String),
}
