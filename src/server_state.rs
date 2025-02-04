use crate::GameObjects;
use std::{collections::HashMap, net::SocketAddr};

pub struct GlobalState {
    players: HashMap<usize, Player>,
    game_objects: GameObjects,
}

impl GlobalState {
    pub fn new() -> GlobalState {
        GlobalState {
            players: HashMap::new(),
            game_objects: GameObjects::new(),
        }
    }

    pub fn add_player(&mut self) {
        // TODO: is it safe to just increment it?
        let player_id = self.players.len();
        let player = Player::new(player_id);
        self.players.insert(player_id, player);
    }
}

#[derive(Debug, Default)]
pub struct Player {
    // metadata
    player_id: usize,
    client_request_id: usize,
    udp_address: Option<SocketAddr>,
    tcp_address: Option<SocketAddr>,
    // game related
    location: (usize, usize), // grid position
}

impl Player {
    pub fn new(player_id: usize) -> Player {
        Player {
            player_id,
            ..Default::default()
        }
    }

    pub fn location(&self) -> (usize, usize) {
        self.location
    }
}
