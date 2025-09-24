mod chat_window;

use chat_window::create_chat_window;
use chrono::{DateTime, Local};
use shared::network::tcp::TcpClientMsg;
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
   pub is_dead: bool,
   pub player_id: uuid::Uuid,
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

      if mmo_ctx.is_dead {
         create_death_dialog(mmo_ctx, egui_ctx);
      }
   });
}

fn create_death_dialog(mmo_ctx: &mut MmoContext, ctx: &egui_macroquad::egui::Context) {
   egui_macroquad::egui::Window::new("You Died")
      .collapsible(false)
      .resizable(false)
      .anchor(egui_macroquad::egui::Align2::CENTER_CENTER, [0.0, 0.0])
      .show(ctx, |ui| {
         ui.vertical_centered(|ui| {
            ui.label("You have been defeated!");
            ui.add_space(10.0);

            ui.horizontal(|ui| {
               if ui.button("Respawn").clicked() {
                  // Send respawn request
                  let respawn_msg = TcpClientMsg::Respawn(mmo_ctx.player_id);
                  if let Ok(serialized) = bincode::serialize(&respawn_msg) {
                     _ = mmo_ctx
                        .server_tcp_write_stream
                        .lock()
                        .unwrap()
                        .try_write(&serialized);
                  }
                  mmo_ctx.is_dead = false;
               }

               if ui.button("Exit").clicked() {
                  // Send disconnect and exit
                  let disconnect_msg = TcpClientMsg::Disconnect;
                  if let Ok(serialized) = bincode::serialize(&disconnect_msg) {
                     _ = mmo_ctx
                        .server_tcp_write_stream
                        .lock()
                        .unwrap()
                        .try_write(&serialized);
                  }
                  std::process::exit(0);
               }
            });
         });
      });
}
