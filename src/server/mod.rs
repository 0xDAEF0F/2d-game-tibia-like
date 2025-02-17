mod player;
pub mod tasks;

use crate::Location;
use crate::constants::*;
use log::debug;
pub use player::*;
use std::collections::HashMap;
use std::collections::HashSet;
use std::collections::VecDeque;
use std::ops::{Index, IndexMut};
use std::time::Instant;
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

#[derive(Debug)]
pub struct MmoMap([[MapElement; MAP_WIDTH as usize]; MAP_HEIGHT as usize]);

impl MmoMap {
    pub fn new() -> MmoMap {
        let map = MmoMap([[MapElement::Empty; MAP_WIDTH as usize]; MAP_HEIGHT as usize]);
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
        let MapElement::Monster(last_movement) = &mut element else {
            debug!("Expected a monster at location {:?}", from);
            return Some(());
        };

        *last_movement = Instant::now();
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
                if !visited.contains(&neighbor) {
                    if let Some(MapElement::Empty) = self.get(neighbor) {
                        queue.push_back(neighbor);
                        visited.insert(neighbor);
                        came_from.insert(neighbor, current);
                    }
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

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MapElement {
    Empty,
    Monster(Instant), // last movement
    Player(Uuid),     // player id
    Object,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_indexing() {
        let mut map = MmoMap::new();
        let location = (5, 5);
        let player_id = Uuid::new_v4();

        // Test setting a player at a location
        map[location] = MapElement::Player(player_id);
        assert_eq!(map[location], MapElement::Player(player_id));

        // Test moving the player to a new location
        let new_location = (6, 5);
        map.move_monster(location, new_location).unwrap();
        assert_eq!(map[new_location], MapElement::Player(player_id));
        assert_eq!(map[location], MapElement::Empty);
    }

    #[test]
    fn test_move_element() {
        let mut map = MmoMap::new();
        let from = (2, 2);
        let to = (3, 3);
        let player_id = Uuid::new_v4();

        map[from] = MapElement::Player(player_id);
        assert_eq!(map[from], MapElement::Player(player_id));
        assert_eq!(map[to], MapElement::Empty);

        map.move_monster(from, to).unwrap();
        assert_eq!(map[to], MapElement::Player(player_id));
        assert_eq!(map[from], MapElement::Empty);
    }

    #[test]
    fn test_shortest_path() {
        let map = MmoMap::new();
        let from = (0, 0);
        let to = (2, 2);

        let path = map.shortest_path(from, to);
        assert_eq!(path, vec![(0, 0), (1, 0), (2, 0), (2, 1), (2, 2)]);
    }

    #[test]
    fn test_get() {
        let mut map = MmoMap::new();
        let location = (1, 1);

        let player = MapElement::Player(Uuid::new_v4());

        map.0[location.1 as usize][location.0 as usize] = player;

        assert_eq!(map.get(location), Some(&player));
    }
}
