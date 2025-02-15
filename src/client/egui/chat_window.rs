use super::{ChatMessage, MmoContext};
use crate::TcpClientMsg;
use egui_macroquad::egui::{self, Key, Modifiers, Pos2};
use egui_macroquad::macroquad::prelude::*;

pub fn create_chat_window(mmo_context: &mut MmoContext, egui_ctx: &egui::Context) {
    let chat = &mut mmo_context.user_chat;
    let text = &mut mmo_context.user_text;
    let tcp_writer = &mmo_context.server_tcp_write_stream;
    egui::Window::new("Chat Box")
        .default_pos(Pos2::new((screen_width()) / 2., screen_height()))
        .resizable([true, true])
        .show(egui_ctx, |ui| {
            ui.horizontal(|ui| ui.label("Messages"));
            ui.add_space(4.);

            let row_height = ui.text_style_height(&egui::TextStyle::Body);
            egui::ScrollArea::vertical()
                .max_height(200.)
                .stick_to_bottom(true)
                .show_rows(ui, row_height, chat.len(), |ui, row_range| {
                    for (_, msg) in row_range.zip(chat.iter()) {
                        ui.label(msg.to_string());
                    }
                });

            // text input
            let text_edit_output = egui::text_edit::TextEdit::singleline(text)
                .hint_text("type text here")
                .show(ui);

            if ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter)) {
                if !text.is_empty() {
                    let msg = TcpClientMsg::ChatMsg(text.clone());
                    let serialized = bincode::serialize(&msg).unwrap();
                    if let Ok(size) = tcp_writer.lock().unwrap().try_write(&serialized) {
                        info!("sent {} bytes", size);
                        info!("sent chat message: {}", text);
                    } else {
                        error!("could not send chat message: {}", text);
                    }
                    chat.push(ChatMessage::new(mmo_context.username.clone(), text.clone()));
                    text.clear();
                    text_edit_output.response.request_focus();
                }
            }
        });
}
