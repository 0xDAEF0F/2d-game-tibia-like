use macroquad::prelude::*;
use std::sync::Arc;
use tiled::Tileset;

/// A container for a tileset and the texture it references.
pub struct Tilesheet {
    texture: Texture2D,
    tileset: Arc<Tileset>,
}

impl Tilesheet {
    /// Create a tilesheet from a Tiled tileset, loading its texture along the way.
    pub fn from_tileset(tileset: Arc<Tileset>) -> Self {
        let tileset_image = tileset.image.as_ref().unwrap();

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

    pub fn texture(&self) -> &Texture2D {
        &self.texture
    }

    pub fn draw_tile_id_at(&self, id: u32, location: (u32, u32)) {
        let (tile_x, tile_y, width, height) = self.tile_rect(id);
        let (loc_x, loc_y) = location;
        draw_texture_ex(
            &self.texture,
            loc_x as f32 * 32.,
            loc_y as f32 * 32.,
            WHITE,
            DrawTextureParams {
                source: Some(Rect::new(tile_x, tile_y, width, height)),
                dest_size: Some(vec2(width, height)),
                ..Default::default()
            },
        );
    }

    pub fn tile_rect(&self, id: u32) -> (f32, f32, f32, f32) {
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
