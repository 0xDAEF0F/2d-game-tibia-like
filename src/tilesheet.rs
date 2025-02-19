use egui_macroquad::macroquad::prelude::*;
use log::info;
use std::sync::Arc;
use tiled::Tileset;

use crate::Location;

/// A container for a tileset and the texture it references.
#[derive(Debug)]
pub struct Tilesheet {
    texture: Texture2D,
    tileset: Arc<Tileset>,
}

impl Tilesheet {
    /// Create a tilesheet from a Tiled tileset, loading its texture along the way.
    pub fn from_tileset(tileset: Arc<Tileset>) -> Tilesheet {
        let tileset_image = tileset.image.as_ref().unwrap();

        info!("loading image: {:?}", tileset_image.source);

        let texture = {
            let texture_path = &tileset_image
                .source
                .to_str()
                .expect("obtaining valid UTF-8 path");
            let bytes = std::fs::read(texture_path).expect("Failed to read texture file");
            Texture2D::from_file_with_format(&bytes, None)
        };

        Tilesheet { texture, tileset }
    }

    pub fn render_tile_at(&self, tile_id: u32, location: Location) {
        let (x_coordinate, y_coordinate) = location;
        let (tile_x, tile_y, width, height) = self.tile_rect(tile_id);
        // 32, 0, 32, 32
        // if tile_id == 2 {
        //     println!("{:?}", self.tile_rect(tile_id));
        // }
        // let tileset_name = &self.tileset.name;
        // debug!("drawing from {tileset_name}",);
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
