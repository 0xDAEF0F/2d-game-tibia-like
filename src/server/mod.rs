mod player;
pub mod tasks;

use crate::{GameObject, GameObjects, Location, constants::*};
use log::debug;
pub use player::*;
use std::{
	collections::{HashMap, HashSet, VecDeque},
	ops::{Index, IndexMut},
	time::Instant,
};
use uuid::Uuid;

pub struct ServerChannel {
	id: Uuid,
	msg: Sc,
}

pub enum Sc {
	PlayerMove {
		client_request_id: u32,
		location: Location,
	},
	Disconnect,
	MoveObject {
		from: Location,
		to: Location,
	},
	ChatMsg(String), // message
	Ping(u32),       // ping_id
}

#[derive(Debug, Default)]
pub struct MmoMap([[MapElement; MAP_WIDTH as usize]; MAP_HEIGHT as usize]);

impl MmoMap {
	pub fn new() -> MmoMap {
		MmoMap([[MapElement::Empty; MAP_WIDTH as usize]; MAP_HEIGHT as usize])
	}

	pub fn from_game_objects(game_objects: GameObjects) -> MmoMap {
		let mut map = MmoMap::new();

		for (location, game_object) in game_objects.get_objects() {
			let map_element = match game_object {
				GameObject::FlowerPot {
					id,
					tileset_location,
				} => MapElement::Object(Object {
					id: (id, tileset_location),
				}),
				GameObject::Orc {
					id,
					tileset_location,
					..
				} => MapElement::Monster(Monster {
					id: (id, tileset_location),
					last_movement: Instant::now(),
					last_attack: Instant::now(),
				}),
			};
			map[location] = map_element;
		}

		map
	}

	pub fn get(&self, (x, y): Location) -> Option<&MapElement> {
		self.0.get(y as usize).and_then(|row| row.get(x as usize))
	}

	pub fn move_monster(&mut self, from: Location, to: Location) -> Option<()> {
		if from == to {
			debug!("Cannot move monster to the same location");
			return Some(());
		}

		let mut element = self[from];
		let MapElement::Monster(monster) = &mut element else {
			debug!("Expected a monster at location {:?}", from);
			return Some(());
		};

		monster.last_movement = Instant::now();

		self[to] = element;
		self[from] = MapElement::Empty;

		Some(())
	}

	pub fn shortest_path(&self, from: Location, to: Location) -> Vec<Location> {
		let mut queue = VecDeque::new();
		let mut visited = HashSet::new();
		let mut came_from = HashMap::new();

		queue.push_back(from);
		visited.insert(from);

		while let Some(current) = queue.pop_front() {
			if current == to {
				let mut path = vec![current];
				while let Some(&prev) = came_from.get(&path[path.len() - 1]) {
					path.push(prev);
				}
				path.reverse();
				return path;
			}

			let neighbors = [
				(current.0.wrapping_sub(1), current.1),
				(current.0 + 1, current.1),
				(current.0, current.1.wrapping_sub(1)),
				(current.0, current.1 + 1),
			];

			for &neighbor in &neighbors {
				if !visited.contains(&neighbor)
					&& let Some(MapElement::Empty) = self.get(neighbor) {
						queue.push_back(neighbor);
						visited.insert(neighbor);
						came_from.insert(neighbor, current);
					}
			}
		}

		vec![]
	}
}

impl Index<Location> for MmoMap {
	type Output = MapElement;
	fn index(&self, loc: Location) -> &Self::Output {
		&self.0[loc.1 as usize][loc.0 as usize]
	}
}

impl IndexMut<Location> for MmoMap {
	fn index_mut(&mut self, loc: Location) -> &mut Self::Output {
		&mut self.0[loc.1 as usize][loc.0 as usize]
	}
}

#[derive(Debug, Copy, Clone, PartialEq, Default)]
pub enum MapElement {
	#[default]
	Empty,
	Monster(Monster), // last movement
	Player(Uuid),     // player id
	Object(Object),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Monster {
	pub id: (u32, usize), // id, tileset_location
	pub last_movement: Instant,
	pub last_attack: Instant,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Object {
	pub id: (u32, usize), // id, tileset_location
}
