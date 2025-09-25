use crate::{Direction, Location, calculate_new_direction, constants::*};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thin_logger::log::trace;
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

      let mut all_objects = HashMap::new();

      // Iterate through groups to get objects with proper z-levels
      for (group_idx, layer) in map.layers().enumerate() {
         if let tiled::LayerType::Group(group_layer) = layer.layer_type() {
            let z_level = group_idx as u32;

            // Find object layers within this group
            for inner_layer in group_layer.layers() {
               if let tiled::LayerType::Objects(object_layer) = inner_layer.layer_type() {
                  for od in object_layer.object_data() {
                     let tile_data = od.tile_data().unwrap();
                     let tiled::TilesetLocation::Map(location) = tile_data.tileset_location()
                     else {
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
                        83 => GameObject::Ladder {
                           id: tile_id,
                           tileset_location: *location,
                           target_z: 1, // Goes up one level
                        },
                        id => todo!("game object id: {id} is not implemented"),
                     };

                     let obj_location = (
                        (od.x / TILE_WIDTH) as u32,
                        (od.y / TILE_HEIGHT) as u32,
                        z_level,
                     );
                     all_objects.insert(obj_location, game_object);
                  }
               }
            }
         }
      }

      GameObjects(all_objects)
   }

   pub fn get_objects(self) -> Vec<(Location, GameObject)> {
      self.0.into_iter().collect()
   }

   pub fn move_object(&mut self, from: Location, to: Location) -> Option<()> {
      let mut object = self.0.remove(&from)?;
      if object.is_monster() {
         let direction = calculate_new_direction(from, to);
         trace!("changing direction of monster to: {:?}", direction);
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
   Ladder {
      id: u32,
      tileset_location: usize,
      target_z: u32,
   },
}

impl GameObject {
   pub fn id(&self) -> u32 {
      match self {
         GameObject::FlowerPot { id, .. } => *id,
         GameObject::Orc { id, .. } => *id,
         GameObject::Ladder { id, .. } => *id,
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
         GameObject::Ladder {
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
         loader.load_tmx_map("../assets/basic-map.tmx").unwrap()
      };

      // Check we have 2 groups
      let layers: Vec<_> = map.layers().collect();
      assert_eq!(layers.len(), 2);

      // First group should be "base" with 2 layers
      let base_group = map.get_layer(0).unwrap();
      assert_eq!(base_group.name, "base");
      if let tiled::LayerType::Group(group_layer) = base_group.layer_type() {
         let base_layers: Vec<_> = group_layer.layers().collect();
         assert_eq!(base_layers.len(), 2);
      } else {
         panic!("Expected base to be a group layer");
      }

      // Second group should be "top" with 1 layer
      let top_group = map.get_layer(1).unwrap();
      assert_eq!(top_group.name, "top");
      if let tiled::LayerType::Group(group_layer) = top_group.layer_type() {
         let top_layers: Vec<_> = group_layer.layers().collect();
         assert_eq!(top_layers.len(), 1);
      } else {
         panic!("Expected top to be a group layer");
      }
   }
}
