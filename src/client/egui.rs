mod chat_window;

use chat_window::create_chat_window;
use tokio::net::tcp::OwnedWriteHalf;

pub struct MmoContext<'a> {
    pub user_text: String,
    pub user_chat: Vec<String>,
    pub server_tcp_write_stream: &'a OwnedWriteHalf,
}

pub fn make_egui(mmo_ctx: &mut MmoContext) {
    egui_macroquad::ui(|egui_ctx| {
        egui_ctx.set_zoom_factor(2.0);
        create_chat_window(mmo_ctx, egui_ctx);
    });
}
