mod player;
pub mod tasks;

use crate::Location;
use crate::constants::*;
pub use player::*;
use std::collections::HashMap;
use std::collections::HashSet;
use std::collections::VecDeque;
use std::ops::{Index, IndexMut};
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

#[derive(Debug, Clone, Copy)]
pub enum MapElement {
    Empty,
    Monster,
    Player(Uuid),
    Object,
}
