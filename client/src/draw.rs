use crate::{
   Cc, ChatMessage, ClientChannel, FpsLogger, GameObjects, Location, MmoContext, MmoTilesheets,
   OtherPlayer, OtherPlayers, PingMonitor, Player, make_egui,
   movement::{check_ladder_interaction, handle_player_movement, send_pos_to_server},
   object_interaction::{handle_end_move_object, handle_start_move_object},
   pathfinding::{handle_route, program_route_if_user_clicks_map},
   rendering::{render_objects, render_view},
   tasks::tcp_reader_task,
};
use egui_macroquad::macroquad::prelude::*;
use shared::{
   constants::{MAX_CONNECTION_RETRIES, SERVER_TCP_ADDR},
   tcp::TcpClientMsg,
   udp::UdpClientMsg,
};
use std::{
   collections::{HashMap, hash_map::Entry},
   net::SocketAddr,
   sync::{Arc, Mutex},
   time::Duration,
};
use thin_logger::log::{debug, info, warn};
use tiled::Loader;
use tokio::{
   io::AsyncWriteExt,
   net::{TcpSocket, TcpStream, UdpSocket},
   sync::mpsc::{UnboundedReceiver, UnboundedSender},
};

pub async fn draw(
   socket: Arc<UdpSocket>,
   tcp_stream: TcpStream,
   mut cc_rx: UnboundedReceiver<ClientChannel>,
   cc_tx: UnboundedSender<ClientChannel>,
   mut player: Player,
) {
   prevent_quit();

   let (tcp_reader, tcp_writer) = tcp_stream.into_split();
   let tcp_writer = Arc::new(Mutex::new(tcp_writer));

   let tcp_writer_ = Arc::clone(&tcp_writer);
   let socket_clone = socket.clone();
   tokio::spawn(async move {
      let jh = tcp_reader_task(tcp_reader, cc_tx.clone(), player.id);
      _ = tokio::join!(jh);
      let mut attempts_to_reconnect = 0;
      while attempts_to_reconnect <= MAX_CONNECTION_RETRIES {
         warn!("attempting re-connection to server (TCP).");
         let tcp_socket = TcpSocket::new_v4().unwrap();
         let server_addr: SocketAddr = SERVER_TCP_ADDR.parse().unwrap();
         let Ok(mut tcp_stream) = tcp_socket.connect(server_addr).await else {
            attempts_to_reconnect += 1;
            tokio::time::sleep(Duration::from_secs(5)).await;
            continue;
         };

         // reconnect to the server
         let reconnect_msg = TcpClientMsg::Reconnect(player.id);
         let reconnect_msg = bincode::serialize(&reconnect_msg).unwrap();
         tcp_stream.write_all(&reconnect_msg).await.unwrap();

         // Send initial UDP ping to re-establish UDP socket on server after reconnection
         let initial_ping = UdpClientMsg::Ping {
            id: player.id,
            client_request_id: 0,
         };
         if let Ok(serialized) = bincode::serialize(&initial_ping) {
            _ = socket_clone.send(&serialized).await;
            debug!("sent initial UDP ping after reconnection");
         }

         let (tcp_reader, tcp_writer_new) = tcp_stream.into_split();
         *tcp_writer_.lock().unwrap() = tcp_writer_new;
         let jh = tcp_reader_task(tcp_reader, cc_tx.clone(), player.id);
         _ = tokio::join!(jh);
      }
      info!("exiting program.");
      std::process::exit(1);
   });

   let mut other_players = OtherPlayers(HashMap::new());

   let map = {
      let mut loader = Loader::new();
      loader.load_tmx_map("assets/basic-map.tmx").unwrap()
   };

   let tilesheets = MmoTilesheets::new(&map);

   let mut game_objects = GameObjects::new();
   let mut moving_object: Option<Location> = None;

   let mut fps_logger = FpsLogger::new();
   let mut ping_monitor = PingMonitor::new();

   let mut is_disconnected = false;

   let mut mmo_context = MmoContext {
      username: player.username.clone(),
      user_text: "".to_string(),
      user_chat: vec![],
      server_tcp_write_stream: tcp_writer.clone(),
      is_dead: false,
      player_id: player.id,
   };

   loop {
      clear_background(Color::from_rgba(31, 31, 31, 255)); // dark gray

      if is_disconnected {
         continue;
      }

      make_egui(&mut mmo_context);

      while let Ok(msg) = cc_rx.try_recv() {
         match msg.msg {
            Cc::PlayerMove {
               client_request_id,
               location,
            } => {
               if client_request_id >= player.request_id {
                  player.prev_location = location;
                  player.curr_location = location;
               } else {
                  player.prev_location = player.curr_location;
               }
            }
            Cc::MoveObject { from, to } => {
               if let Some(val) = game_objects.0.remove(&from) {
                  game_objects.0.insert(to, val);
               }
            }
            Cc::ChatMsg { from, msg } => {
               debug!("received message from: {from}. pushing it to the chat.");
               mmo_context.user_chat.push(ChatMessage::new(from, msg));
            }
            Cc::Pong(ping_id) => ping_monitor.log_ping(&ping_id),
            Cc::Objects(game_obj) => {
               game_objects = game_obj;
            }
            Cc::Disconnect => {
               is_disconnected = true;
            }
            Cc::ReconnectOk => {
               is_disconnected = false;
            }
            Cc::OtherPlayer(op) => match other_players.0.entry(op.username.clone()) {
               Entry::Occupied(mut entry) => {
                  let player = entry.get_mut();
                  if player.location != op.location || player.direction != op.direction {
                     player.frame = (player.frame + 1) % 3;
                     player.location = op.location;
                     player.direction = op.direction;
                  }
               }
               Entry::Vacant(entry) => {
                  entry.insert(OtherPlayer::new(op.username, op.location, op.direction));
               }
            },
            Cc::PlayerHealthUpdate { hp } => {
               player.hp = hp;
               debug!("Health updated: {}/{}", hp, player.max_hp);
            }
            Cc::PlayerDeath { message } => {
               info!("Player died: {}", message);
               mmo_context.is_dead = true;
            }
            Cc::RespawnOk { hp, location } => {
               info!("Respawned at location {:?} with {} HP", location, hp);
               player.hp = hp;
               player.curr_location = location;
               player.prev_location = location;
               player.route.clear();
               mmo_context.is_dead = false;
            }
         }
      }

      render_view(&player, &map, &tilesheets);

      // Only render player sprite if alive
      if !mmo_context.is_dead {
         player.render(&tilesheets);
      }

      other_players.render(&player, &tilesheets);

      render_objects(&player, &tilesheets, &game_objects);

      // Skip player interactions if dead
      if !mmo_context.is_dead {
         program_route_if_user_clicks_map(&mut player, &game_objects, &other_players);

         handle_route(&mut player, &game_objects, &other_players);

         handle_player_movement(&mut player, &other_players);

         // Check for ladder interaction after movement
         check_ladder_interaction(&mut player, &game_objects);

         // Object movements
         handle_start_move_object(&game_objects, &mut moving_object, &player);
         handle_end_move_object(&mut game_objects, &mut moving_object, &player, &socket);

         // Send player state to server if changed
         send_pos_to_server(&mut player, &socket);
      }

      fps_logger.log_fps();
      ping_monitor.ping_server(&tcp_writer);

      egui_macroquad::draw();

      if is_quit_requested() {
         let ser = bincode::serialize(&TcpClientMsg::Disconnect).unwrap();
         _ = tcp_writer.lock().unwrap().try_write(&ser);
         info!("shutting down client program.");
         std::process::exit(0);
      }

      next_frame().await;
   }
}
