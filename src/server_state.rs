use crate::{GameObject, GameObjects, Location};
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
        let player_id = self.create_new_player_id();
        let player = Player::new(player_id);
        self.players.insert(player_id, player);
    }

    pub fn get_all_players_locations(&self) -> Vec<(usize, Location)> {
        self.players
            .iter()
            .map(|(&id, p)| (id, p.location))
            .collect()
    }

    pub fn get_all_game_objects(&self) -> Vec<(GameObject, Location)> {
        self.game_objects.0.iter().map(|(&l, &g)| (g, l)).collect()
    }

    // TODO: add an id generator
    fn create_new_player_id(&self) -> usize {
        self.players.len()
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
    location: Location, // grid position. TODO: add y axis for levels
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
