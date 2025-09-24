use crate::{Direction, Location, calculate_new_direction, constants::*};
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tiled::Loader;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct GameObjects(pub HashMap<Location, GameObject>);

#[allow(clippy::new_without_default)]
impl GameObjects {
   pub fn new() -> GameObjects {
      let map = {
         let mut loader = Loader::new();
         loader.load_tmx_map("assets/basic-map.tmx").unwrap()
      };

      let objects = map
         .layers()
         .filter_map(|layer| match layer.layer_type() {
            tiled::LayerType::Objects(object_layer) => Some(object_layer),
            _ => None,
         })
         .collect_vec();

      let objects = objects[0].object_data();

      let objects = objects.iter().map(|od| {
         let tile_data = od.tile_data().unwrap();
         let tiled::TilesetLocation::Map(location) = tile_data.tileset_location() else {
            panic!("Invalid tileset location layer!");
         };
         let tile_id = od.tile_data().expect("expected tile data").id();

         let game_object = match tile_id {
            149 => GameObject::FlowerPot {
               id: tile_id,
               tileset_location: *location,
            },
            63 => GameObject::Orc {
               id: tile_id,
               tileset_location: *location,
               hp: 100,
               direction: Direction::South,
            },
            id => todo!("game object id: {id} is not implemented"),
         };

         (
            ((od.x / TILE_WIDTH) as u32, (od.y / TILE_HEIGHT) as u32),
            game_object,
         )
      });
      let objects: HashMap<Location, GameObject> = HashMap::from_iter(objects);

      GameObjects(objects)
   }

   pub fn get_objects(self) -> Vec<(Location, GameObject)> {
      self.0.into_iter().collect()
   }

   pub fn move_object(&mut self, from: Location, to: Location) -> Option<()> {
      let mut object = self.0.remove(&from)?;
      if object.is_monster() {
         let direction = calculate_new_direction(from, to);
         log::trace!("changing direction of monster to: {:?}", direction);
         object.change_direction(direction);
      }
      self.0.insert(to, object);
      Some(())
   }
}

pub const ORC_MAX_HP: u32 = 100;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum GameObject {
   FlowerPot {
      id: u32,
      tileset_location: usize,
   },
   Orc {
      id: u32,
      tileset_location: usize,
      hp: u32,
      direction: Direction,
   },
}

impl GameObject {
   pub fn id(&self) -> u32 {
      match self {
         GameObject::FlowerPot { id, .. } => *id,
         GameObject::Orc { id, .. } => *id,
      }
   }

   pub fn is_monster(&self) -> bool {
      matches!(self, GameObject::Orc { .. })
   }

   pub fn change_direction(&mut self, direction: Direction) {
      if let GameObject::Orc { direction: d, .. } = self {
         *d = direction
      }
   }

   pub fn tileset_location(&self) -> usize {
      match self {
         GameObject::FlowerPot {
            tileset_location, ..
         } => *tileset_location,
         GameObject::Orc {
            tileset_location, ..
         } => *tileset_location,
      }
   }
}

#[cfg(test)]
mod tests {
   use super::*;

   #[test]
   fn test_load_map() {
      let map = {
         let mut loader = Loader::new();
         loader.load_tmx_map("assets/basic-map.tmx").unwrap()
      };

      let layer = map.get_layer(0).unwrap();

      assert_eq!(layer.name, "Tile Layer 1");
   }
}
