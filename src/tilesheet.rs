use crate::Location;
use egui_macroquad::macroquad::prelude::*;
use log::info;
use std::{collections::HashMap, hash::Hash, sync::Arc};
use tiled::{Layer, LayerType, Map, ObjectLayer, TileLayer, Tileset};

/// A container for a tileset and the texture it references.
#[derive(Debug)]
pub struct Tilesheet {
    texture: Texture2D,
    tileset: Arc<Tileset>,
}

impl Tilesheet {
    pub fn name(&self) -> &str {
        &self.tileset.name
    }

    /// Create a tilesheet from a Tiled tileset, loading its texture along the way.
    pub fn from_tileset(tileset: Arc<Tileset>) -> Tilesheet {
        Tilesheet {
            texture: texture_from_tileset(&tileset),
            tileset,
        }
    }

    pub fn render_tile_at(&self, tile_id: u32, location: Location) {
        let (x_coordinate, y_coordinate) = location;
        let (tile_x, tile_y, width, height) = self.tile_rect(tile_id);
        draw_texture_ex(
            &self.texture,
            x_coordinate as f32 * width,
            y_coordinate as f32 * height,
            WHITE,
            DrawTextureParams {
                source: Some(Rect::new(tile_x, tile_y, width, height)),
                dest_size: Some(vec2(width, height)),
                ..Default::default()
            },
        );
    }

    fn tile_rect(&self, id: u32) -> (f32, f32, f32, f32) {
        let tile_width = self.tileset.tile_width;
        let tile_height = self.tileset.tile_height;
        let spacing = self.tileset.spacing;
        let margin = self.tileset.margin;
        let tiles_per_row = (self.tileset.image.as_ref().unwrap().width as u32 - margin + spacing)
            / (tile_width + spacing);
        let x = id % tiles_per_row * tile_width;
        let y = id / tiles_per_row * tile_height;

        (x as f32, y as f32, tile_width as f32, tile_height as f32)
    }
}

// Experimental

pub struct MmoTilesheets<'tileset> {
    layers: HashMap<&'tileset str, (Arc<Tileset>, Texture2D)>,
}

impl MmoTilesheets<'_> {
    pub fn new(map: &Map) -> MmoTilesheets {
        let mut layers = HashMap::new();

        for tileset in map.tilesets() {
            info!("loading tileset: {:?}", tileset.name);
            let texture = texture_from_tileset(tileset);
            layers.insert(tileset.name.as_str(), (tileset.clone(), texture));
        }

        MmoTilesheets { layers }
    }

    pub fn render_tile_at(&self, tileset_name: &str, tile_id: u32, location: Location) {
        let (x_coordinate, y_coordinate) = location;
        let Some((tileset, texture)) = self.layers.get(tileset_name) else {
            error!("tileset not found: {:?}", tileset_name);
            return;
        };
        let (tile_x, tile_y, width, height) = Self::tile_rect(tileset, tile_id);
        draw_texture_ex(
            texture,
            x_coordinate as f32 * width,
            y_coordinate as f32 * height,
            WHITE,
            DrawTextureParams {
                source: Some(Rect::new(tile_x, tile_y, width, height)),
                dest_size: Some(vec2(width, height)),
                ..Default::default()
            },
        );
    }

    fn tile_rect(tileset: &Tileset, id: u32) -> (f32, f32, f32, f32) {
        let tile_width = tileset.tile_width;
        let tile_height = tileset.tile_height;
        let spacing = tileset.spacing;
        let margin = tileset.margin;
        let tiles_per_row = (tileset.image.as_ref().unwrap().width as u32 - margin + spacing)
            / (tile_width + spacing);
        let x = id % tiles_per_row * tile_width;
        let y = id / tiles_per_row * tile_height;

        (x as f32, y as f32, tile_width as f32, tile_height as f32)
    }
}

pub fn texture_from_tileset(tileset: &Tileset) -> Texture2D {
    let image = tileset.image.as_ref().expect("tileset has no image");

    info!("loading image: {:?}", image.source);

    let bytes = std::fs::read(image.source.clone()).expect("Failed to read texture file");

    Texture2D::from_file_with_format(&bytes, None)
}
