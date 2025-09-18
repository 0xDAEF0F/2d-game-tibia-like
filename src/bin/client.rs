use anyhow::Result;
use egui_macroquad::macroquad;
use log::{debug, error, info};
use macroquad::{Window, prelude::*};
use my_mmo::{
	client::{
		Cc, ChatMessage, ClientChannel, MmoContext, OtherPlayer, OtherPlayers, Player,
		make_egui, render_entity_name,
		tasks::{tcp_reader_task, udp_recv_task},
	},
	constants::*,
	sendable::SendableSync,
	server::Direction,
	tcp::{TcpClientMsg, TcpServerMsg},
	udp::UdpClientMsg,
	*,
};
use std::{
	collections::{HashMap, VecDeque, hash_map::Entry},
	net::SocketAddr,
	sync::{Arc, Mutex},
	time::Duration,
};
use tiled::{Loader, Map};
use tokio::{
	io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader, stdin},
	net::{TcpSocket, TcpStream, UdpSocket},
	sync::mpsc::{self, UnboundedReceiver, UnboundedSender},
};

#[tokio::main]
async fn main() -> Result<()> {
	MmoLogger::init("debug");

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

async fn draw(
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
	};

	loop {
		clear_background(color_u8!(31, 31, 31, 0)); // dark gray

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
						if player.location != op.location
							|| player.direction != op.direction
						{
							player.frame = (player.frame + 1) % 3;
							player.location = op.location;
							player.direction = op.direction;
						}
					}
					Entry::Vacant(entry) => {
						entry.insert(OtherPlayer::new(
							op.username,
							op.location,
							op.direction,
						));
					}
				},
				Cc::PlayerHealthUpdate { hp } => {
					player.hp = hp;
					debug!("Health updated: {}/{}", hp, player.max_hp);
				}
			}
		}

		render_view(&player, &map, &tilesheets);

		player.render(&tilesheets);

		other_players.render(&player, &tilesheets);

		render_objects(&player, &tilesheets, &game_objects);

		program_route_if_user_clicks_map(&mut player, &game_objects, &other_players);

		handle_route(&mut player, &game_objects, &other_players);

		handle_player_movement(&mut player, &other_players);

		// Object movements
		handle_start_move_object(&game_objects, &mut moving_object, &player);
		handle_end_move_object(&mut game_objects, &mut moving_object, &player, &socket);

		// Send player state to server if changed
		send_pos_to_server(&mut player, &socket);

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

fn send_pos_to_server(player: &mut Player, socket: &UdpSocket) {
	if player.curr_location == player.prev_location {
		return;
	}

	let msg = UdpClientMsg::PlayerMove {
		id: player.id,
		client_request_id: {
			player.request_id += 1;
			player.request_id
		},
		location: player.curr_location,
	};
	socket.send_msg_and_log(&msg, None);
}

fn handle_player_movement(player: &mut Player, op: &OtherPlayers) {
	let current_time = get_time();
	let can_move = current_time - player.last_move_timer >= player.speed.into();

	if !can_move {
		return;
	}

	let mut keys_down = get_keys_down();

	if keys_down.len() == 1 {
		handle_single_key_movement(
			player,
			op,
			keys_down.drain().next().unwrap(),
			current_time,
		);
	} else if keys_down.len() == 2 {
		handle_double_key_movement(player, op, current_time);
	}
}

fn handle_single_key_movement(
	player: &mut Player,
	op: &OtherPlayers,
	key: KeyCode,
	current_time: f64,
) {
	let (x, y) = player.curr_location;
	let (x, y) = (x as i32, y as i32);
	match key {
		KeyCode::Right if Player::can_move((x + 1, y), op) => {
			move_player(player, (1, 0), current_time, BASE_MOVE_DELAY);
		}
		KeyCode::Left if Player::can_move((x - 1, y), op) => {
			move_player(player, (-1, 0), current_time, BASE_MOVE_DELAY);
		}
		KeyCode::Up if Player::can_move((x, y - 1), op) => {
			move_player(player, (0, -1), current_time, BASE_MOVE_DELAY);
		}
		KeyCode::Down if Player::can_move((x, y + 1), op) => {
			move_player(player, (0, 1), current_time, BASE_MOVE_DELAY);
		}
		_ => {}
	}
}

fn handle_double_key_movement(player: &mut Player, op: &OtherPlayers, current_time: f64) {
	let (x, y) = player.curr_location;
	let (x, y) = (x as i32, y as i32);
	if is_key_down(KeyCode::Right)
		&& is_key_down(KeyCode::Up)
		&& Player::can_move((x + 1, y - 1), op)
	{
		move_player(player, (1, -1), current_time, BASE_MOVE_DELAY * 2.0);
	}
	if is_key_down(KeyCode::Right)
		&& is_key_down(KeyCode::Down)
		&& Player::can_move((x + 1, y + 1), op)
	{
		move_player(player, (1, 1), current_time, BASE_MOVE_DELAY * 2.0);
	}
	if is_key_down(KeyCode::Left)
		&& is_key_down(KeyCode::Up)
		&& Player::can_move((x - 1, y - 1), op)
	{
		move_player(player, (-1, -1), current_time, BASE_MOVE_DELAY * 2.0);
	}
	if is_key_down(KeyCode::Left)
		&& is_key_down(KeyCode::Down)
		&& Player::can_move((x - 1, y + 1), op)
	{
		move_player(player, (-1, 1), current_time, BASE_MOVE_DELAY * 2.0);
	}
}

fn move_player(
	player: &mut Player,
	direction: (isize, isize),
	current_time: f64,
	speed: f32,
) {
	player.prev_location = player.curr_location;
	player.curr_location.0 = (player.curr_location.0 as isize + direction.0) as u32;
	player.curr_location.1 = (player.curr_location.1 as isize + direction.1) as u32;
	player.last_move_timer = current_time;
	player.speed = speed;

	debug!("moving player to {:?}", player.curr_location);

	let direction = match direction {
		(1, 0) => Direction::East,
		(-1, 0) => Direction::West,
		(0, -1) => Direction::North,
		(0, 1) => Direction::South,
		(_, 1) => Direction::South,
		(_, -1) => Direction::North,
		(1, _) => Direction::East,
		(-1, _) => Direction::West,
		_ => unreachable!(),
	};

	player.direction = direction;
	player.frame = (player.frame + 1) % 3; // Cycle through frames 0, 1, 2
}

/// Renders the camera around the player
fn render_view(player: &Player, map: &Map, tilesheets: &MmoTilesheets) {
	for i in 0..CAMERA_HEIGHT {
		for j in 0..CAMERA_WIDTH {
			let x = player.curr_location.0 as i32 - CAMERA_WIDTH as i32 / 2 + j as i32;
			let y = player.curr_location.1 as i32 - CAMERA_HEIGHT as i32 / 2 + i as i32;

			let tile_id = map
				.get_layer(0)
				.and_then(|l| l.as_tile_layer())
				.and_then(|tl| tl.get_tile(x, y))
				.and_then(|t| t.id().into());

			if let Some(t_id) = tile_id {
				tilesheets.render_tile_at("grass-tileset", t_id, (j, i));
			} else {
				draw_rectangle(
					j as f32 * TILE_HEIGHT,
					i as f32 * TILE_WIDTH,
					TILE_WIDTH,
					TILE_HEIGHT,
					BLACK,
				);
			}
		}
	}
}

fn render_objects(
	player: &Player,
	tilesheets: &MmoTilesheets,
	game_objects: &GameObjects,
) {
	for i in 0..CAMERA_HEIGHT {
		for j in 0..CAMERA_WIDTH {
			let x = player.curr_location.0 as i32 - CAMERA_WIDTH as i32 / 2 + j as i32;
			let y = player.curr_location.1 as i32 - CAMERA_HEIGHT as i32 / 2 + i as i32;

			if x.is_negative() || y.is_negative() {
				continue;
			}

			let (x, y) = (x as u32, y as u32);

			if !game_objects.0.contains_key(&(x, y)) {
				continue;
			}

			let game_object = &game_objects.0[&(x, y)];

			if let GameObject::Orc { hp, direction, .. } = game_object {
				log::trace!("Orc direction is: {direction:?}",);

				render_entity_name(
					"Orc",
					(j as f32 * TILE_WIDTH, i as f32 * TILE_HEIGHT),
				);

				let healthbar_pct: f32 = *hp as f32 / ORC_MAX_HP as f32;

				let bar_width = 32.0;
				let bar_height = 4.0;
				let offset_y = -6.0; // move the health bar slightly above the Orc tile

				// background
				draw_rectangle(
					j as f32 * TILE_WIDTH,
					i as f32 * TILE_HEIGHT + offset_y,
					bar_width,
					bar_height,
					RED,
				);

				// fill
				draw_rectangle(
					j as f32 * TILE_WIDTH,
					i as f32 * TILE_HEIGHT + offset_y,
					bar_width * healthbar_pct,
					bar_height,
					GREEN,
				);

				let tile_id = match direction {
					Direction::South => 63,
					Direction::North => 66,
					Direction::East => 69,
					Direction::West => 72,
				};

				tilesheets.render_tile_at("tibia-sprites", tile_id, (j, i));
				continue;
			}

			tilesheets.render_tile_at("props-tileset", game_object.id(), (j, i));
		}
	}
}

fn handle_start_move_object(
	game_objects: &GameObjects,
	moving_object: &mut Option<Location>,
	player: &Player,
) {
	if moving_object.is_some() {
		return;
	}

	if !is_mouse_button_down(MouseButton::Left) {
		return;
	};

	let (x, y) = mouse_position();

	if x < 0. || y < 0. {
		return;
	}

	let (x, y) = ((x / 32.) as usize, (y / 32.) as usize);

	if x >= CAMERA_WIDTH as usize || y >= CAMERA_HEIGHT as usize {
		return;
	}

	let player_x = player.curr_location.0 as i32 - CAMERA_WIDTH as i32 / 2;
	let player_y = player.curr_location.1 as i32 - CAMERA_HEIGHT as i32 / 2;

	let abs_x = player_x + x as i32;
	let abs_y = player_y + y as i32;

	let (x, y) = (abs_x as u32, abs_y as u32);

	if !game_objects.0.contains_key(&(x, y)) {
		return;
	}

	// check if the object is adjacent to the player
	let (obj_x, obj_y) = (x as i32, y as i32);
	let (player_x, player_y) =
		(player.curr_location.0 as i32, player.curr_location.1 as i32);

	let is_adjacent = (obj_x - player_x).abs() <= 1 && (obj_y - player_y).abs() <= 1;
	if !is_adjacent {
		return;
	}

	*moving_object = Some((x, y));
}

fn handle_end_move_object(
	game_objects: &mut GameObjects,
	moving_object: &mut Option<Location>,
	player: &Player,
	socket: &UdpSocket,
) {
	if moving_object.is_none() || !is_mouse_button_released(MouseButton::Left) {
		return;
	}

	let (x, y) = mouse_position();
	if x < 0. || y < 0. {
		return;
	}

	let (x, y) = ((x / TILE_WIDTH) as usize, (y / TILE_HEIGHT) as usize);
	if x >= CAMERA_WIDTH as usize || y >= CAMERA_HEIGHT as usize {
		return;
	}

	let player_x = player.curr_location.0 as i32 - CAMERA_WIDTH as i32 / 2;
	let player_y = player.curr_location.1 as i32 - CAMERA_HEIGHT as i32 / 2;
	let (abs_x, abs_y) = (player_x + x as i32, player_y + y as i32);
	let (x, y) = (abs_x as u32, abs_y as u32);

	if let Some(moving_obj) = moving_object.take()
		&& let Some(obj) = game_objects.0.remove(&moving_obj)
	{
		debug!(
			"sending moving object from {:?} to {:?}",
			moving_obj,
			(x, y)
		);

		game_objects.0.insert((x, y), obj);
		let msg = UdpClientMsg::MoveObject {
			id: player.id,
			from: moving_obj,
			to: (x, y),
		};
		socket.send_msg_and_log(&msg, None);
	}
}

async fn request_new_session_from_server(
	tcp_stream: &mut TcpStream,
) -> Result<InitPlayer> {
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
		let Ok(init_msg) = bincode::serialize(&TcpClientMsg::Init(username.clone()))
		else {
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

type QuickMap = [[bool; MAP_WIDTH as usize]; MAP_HEIGHT as usize];
fn construct_map_from_unwalkable_objects(
	game_objects: &GameObjects,
	other_players: &OtherPlayers,
) -> QuickMap {
	let mut map = [[true; MAP_WIDTH as usize]; MAP_HEIGHT as usize];
	for (location, game_object) in game_objects.0.iter() {
		if let GameObject::Orc { .. } = game_object {
			map[location.1 as usize][location.0 as usize] = false;
		}
	}
	for player in other_players.0.values() {
		map[player.location.1 as usize][player.location.0 as usize] = false;
	}
	map
}

fn bfs_find_path(map: &QuickMap, start: Location, end: Location) -> Vec<Location> {
	use std::collections::{HashSet, VecDeque};

	let mut queue = VecDeque::new();
	let mut visited = HashSet::new();
	let mut came_from: HashMap<Location, Location> = HashMap::new();

	queue.push_back(start);
	visited.insert(start);

	while let Some(current) = queue.pop_front() {
		if current == end {
			// Reconstruct path
			let mut path = vec![current];
			let mut pos = current;
			while pos != start {
				pos = came_from[&pos];
				path.push(pos);
			}
			path.reverse();
			// Remove start location from path
			path.remove(0);
			return path;
		}

		// Check all adjacent tiles
		let possible_moves = [
			(current.0.wrapping_sub(1), current.1), // Left
			(current.0.wrapping_add(1), current.1), // Right
			(current.0, current.1.wrapping_sub(1)), // Up
			(current.0, current.1.wrapping_add(1)), // Down
		];

		for next in possible_moves {
			// Check if position is within bounds
			if next.0 >= MAP_WIDTH || next.1 >= MAP_HEIGHT {
				continue;
			}

			// Check if position is walkable and not visited
			if map[next.1 as usize][next.0 as usize] && !visited.contains(&next) {
				queue.push_back(next);
				visited.insert(next);
				came_from.insert(next, current);
			}
		}
	}

	vec![]
}

// Handle movement
fn program_route_if_user_clicks_map(
	player: &mut Player,
	game_objects: &GameObjects,
	other_players: &OtherPlayers,
) {
	if !is_mouse_button_pressed(MouseButton::Left) {
		return;
	}

	let Some((x, y)) = get_mouse_map_tile_position(player.curr_location) else {
		return;
	};

	let map = construct_map_from_unwalkable_objects(game_objects, other_players);
	let path = bfs_find_path(&map, player.curr_location, (x, y));

	log::info!("path: {:?}", path);

	if path.is_empty() {
		return;
	}

	player.route = VecDeque::from(path);
}

/// Processes the player's auto-pathing route by moving to the next location
/// in the route queue. Skips movement if blocked by monsters and respects
/// the player's movement speed cooldown.
fn handle_route(
	player: &mut Player,
	game_objects: &GameObjects,
	other_players: &OtherPlayers,
) {
	if player.route.is_empty() {
		return;
	}

	let current_time = get_time();
	let can_move = current_time - player.last_move_timer >= player.speed.into();

	if !can_move {
		return;
	}

	let next_location = player.route.front().unwrap();

	// TODO: there might be other objects on the path that you can't move
	// through
	if let Some(obj) = game_objects.0.get(next_location)
		&& obj.is_monster()
	{
		return;
	}

	let key = match (
		next_location.0 as isize - player.curr_location.0 as isize,
		next_location.1 as isize - player.curr_location.1 as isize,
	) {
		(0, -1) => KeyCode::Up,
		(0, 1) => KeyCode::Down,
		(1, 0) => KeyCode::Right,
		(-1, 0) => KeyCode::Left,
		_ => return,
	};

	handle_single_key_movement(player, other_players, key, get_time());

	player.route.pop_front();
}
