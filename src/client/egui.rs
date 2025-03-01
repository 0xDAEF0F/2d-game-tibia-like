mod chat_window;

use chat_window::create_chat_window;
use chrono::{DateTime, Local};
use std::{
    fmt,
    sync::{Arc, Mutex},
};
use tokio::net::tcp::OwnedWriteHalf;

pub struct MmoContext {
    pub username: String,
    pub user_text: String,
    pub user_chat: Vec<ChatMessage>,
    pub server_tcp_write_stream: Arc<Mutex<OwnedWriteHalf>>,
}

pub struct ChatMessage {
    username: String,
    message: String,
    timestamp: DateTime<Local>,
}

impl ChatMessage {
    pub fn new(username: String, message: String) -> Self {
        Self {
            username,
            message,
            timestamp: Local::now(),
        }
    }
}

impl fmt::Display for ChatMessage {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{} {}: {}",
            self.timestamp.format("%I:%M%p"),
            self.username,
            self.message
        )
    }
}

pub fn make_egui(mmo_ctx: &mut MmoContext) {
    egui_macroquad::ui(|egui_ctx| {
        egui_ctx.set_zoom_factor(2.0);
        create_chat_window(mmo_ctx, egui_ctx);
    });
}
