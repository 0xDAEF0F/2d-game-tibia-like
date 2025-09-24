use anyhow::Result;
use client::{ClientChannel, Player, draw::draw, tasks::udp_recv_task};
use egui_macroquad::macroquad;
use macroquad::{Window, prelude::*};
use shared::{
   constants::*,
   network::{
      tcp::{TcpClientMsg, TcpServerMsg},
      udp::UdpClientMsg,
   },
   *,
};
use std::{collections::VecDeque, sync::Arc};
use thin_logger::log::{LevelFilter, debug, error, info};
use tokio::{
   io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader, stdin},
   net::{TcpSocket, TcpStream, UdpSocket},
   sync::mpsc,
};

#[tokio::main]
async fn main() -> Result<()> {
   thin_logger::build(LevelFilter::Info.into()).init();

   let socket = UdpSocket::bind("0.0.0.0:0").await?;
   let socket = Arc::new(socket);
   socket.connect(SERVER_UDP_ADDR).await?;

   let tcp_socket = TcpSocket::new_v4()?;
   let mut stream = tcp_socket.connect(SERVER_TCP_ADDR.parse()?).await?;

   let init_player = request_new_session_from_server(&mut stream).await?;

   println!(
      "username: {} was accepted by the server.",
      init_player.username
   );

   info!("client connected to server at: {}", SERVER_TCP_ADDR);

   // Send initial UDP ping to establish UDP socket on server
   let initial_ping = UdpClientMsg::Ping {
      id: init_player.id,
      client_request_id: 0,
   };
   let serialized = bincode::serialize(&initial_ping)?;
   socket.send(&serialized).await?;
   debug!("sent initial UDP ping to establish connection");

   let (cc_tx, cc_rx) = mpsc::unbounded_channel::<ClientChannel>();

   // Spawn async UDP receive task
   let join_handle1 = udp_recv_task(socket.clone(), cc_tx.clone(), init_player.id);

   // UDP task is not supposed to finish
   tokio::spawn(async move {
      _ = tokio::join!(join_handle1);
      error!("UDP receive task failed");
      std::process::exit(1);
   });

   // Macroquad configuration and window
   let conf = Conf {
      window_title: "MMO Game".to_string(),
      high_dpi: true,
      ..Default::default()
   };

   let player = Player {
      id: init_player.id,
      username: init_player.username,
      level: init_player.level,
      hp: init_player.hp,
      max_hp: init_player.max_hp,
      frame: 0,
      request_id: 0,
      speed: BASE_MOVE_DELAY,
      curr_location: init_player.location,
      prev_location: init_player.location,
      route: VecDeque::new(),
      last_move_timer: 0.0,
      direction: init_player.direction,
   };

   Window::from_config(conf, draw(socket, stream, cc_rx, cc_tx, player));

   Ok(())
}

// TODO: temporary way to connect to the server via cli
async fn request_new_session_from_server(tcp_stream: &mut TcpStream) -> Result<InitPlayer> {
   loop {
      // request user's username for the session
      let mut username = String::new();
      while username.is_empty() {
         println!("Please enter your desired username for the session.");
         let mut reader = BufReader::new(stdin()).lines();
         let Ok(Some(line)) = reader.next_line().await else {
            println!("invalid username. try again.");
            continue;
         };
         let trimmed = line.trim_ascii();
         if trimmed.len() < 4 {
            println!("username too short. try again.");
            continue;
         }
         username = trimmed.to_string();
      }

      // send the username to the server
      let Ok(init_msg) = bincode::serialize(&TcpClientMsg::Init(username.clone())) else {
         println!("failed to serialize message. try again.");
         continue;
      };
      if tcp_stream.write_all(&init_msg).await.is_err() {
         println!("failed to send initiation msg to server. try again.");
         continue;
      }

      // receive response from server
      let mut buf = [0; 1024];

      let Ok(bytes_received) = tcp_stream.read(&mut buf).await else {
         println!("failed to read msg from server. try again.");
         continue;
      };

      // deserialize server msg
      let Ok(sm) = bincode::deserialize::<TcpServerMsg>(&buf[..bytes_received]) else {
         println!("failed to deserialize server msg. try again");
         continue;
      };

      if let TcpServerMsg::InitErr(err) = sm {
         println!("{}", err);
         println!("retrying everything.");
         continue;
      }

      // make sure response is what's expected
      let TcpServerMsg::InitOk(init_player) = sm else {
         println!("expecting an init ok. retrying everything");
         continue;
      };

      return Ok(init_player);
   }
}
