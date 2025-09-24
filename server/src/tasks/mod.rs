mod game_loop;
mod sc_rx;
mod tcp_listener;
mod udp_recv;

use super::Player;
pub use game_loop::*;
pub use sc_rx::*;
use std::{collections::HashMap, sync::Arc};
pub use tcp_listener::*;
use tokio::sync::Mutex;
pub use udp_recv::*;
use uuid::Uuid;

pub type Players = Arc<Mutex<HashMap<Uuid, Player>>>;
