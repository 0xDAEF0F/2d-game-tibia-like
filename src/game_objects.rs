use crate::{Location, constants::*};
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tiled::Loader;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct GameObjects(pub HashMap<Location, GameObject>);

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
            (
                ((od.x / TILE_WIDTH) as u32, (od.y / TILE_HEIGHT) as u32),
                od.tile_data().expect("expected tile data").id().into(),
            )
        });
        let objects: HashMap<Location, GameObject> = HashMap::from_iter(objects);

        GameObjects(objects)
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum GameObject {
    FlowerPot { id: u32 },
}

impl From<u32> for GameObject {
    fn from(id: u32) -> GameObject {
        match id {
            149 => GameObject::FlowerPot { id },
            id => todo!("{id} not implemented"),
        }
    }
}

impl From<&GameObject> for u32 {
    fn from(val: &GameObject) -> Self {
        match val {
            GameObject::FlowerPot { id } => *id,
        }
    }
}
