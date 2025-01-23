use macroquad::prelude::*;

const TILE_WIDTH: f32 = 32.0;
const TILE_HEIGHT: f32 = 32.0;
const BASE_MOVE_DELAY: f32 = 0.2; // Base delay for movement speed
const GRID_COLOR: Color = color_u8!(200, 200, 200, 255); // Light gray grid color

#[derive(Debug, Default)]
struct Player {
    x: usize,
    y: usize,
    last_move_timer: f64,
    speed: f32, // Movement speed (lower value = faster movement)
}

#[macroquad::main("BasicShapes")]
async fn main() {
    let mut player = Player {
        speed: BASE_MOVE_DELAY,
        last_move_timer: get_time(),
        ..Default::default()
    };

    loop {
        clear_background(color_u8!(31, 31, 31, 0));

        draw_rectangle(
            player.x as f32 * TILE_WIDTH,
            player.y as f32 * TILE_HEIGHT,
            TILE_WIDTH,
            TILE_HEIGHT,
            RED,
        );

        // delimitator lines
        for i in (0..(screen_width() as usize)).step_by(TILE_WIDTH as usize) {
            draw_line(i as f32, 0.0, i as f32, screen_height(), 1.0, GRID_COLOR);
        }
        for j in (0..(screen_height() as usize)).step_by(TILE_HEIGHT as usize) {
            draw_line(0.0, j as f32, screen_width(), j as f32, 1.0, GRID_COLOR);
        }

        let keys_down = get_keys_down();
        let current_time = get_time();
        let can_move = current_time - player.last_move_timer >= player.speed.into();

        if keys_down.len() == 1 && can_move {
            if is_key_down(KeyCode::Right) && player.x < (screen_width() / TILE_WIDTH) as usize - 1
            {
                player.x += 1;
                player.last_move_timer = current_time;
                player.speed = BASE_MOVE_DELAY;
            }
            if is_key_down(KeyCode::Left) && player.x > 0 {
                player.x -= 1;
                player.last_move_timer = current_time;
                player.speed = BASE_MOVE_DELAY;
            }
            if is_key_down(KeyCode::Up) && player.y > 0 {
                player.y -= 1;
                player.last_move_timer = current_time;
                player.speed = BASE_MOVE_DELAY;
            }
            if is_key_down(KeyCode::Down) && player.y < (screen_height() / TILE_HEIGHT) as usize - 1
            {
                player.y += 1;
                player.last_move_timer = current_time;
                player.speed = BASE_MOVE_DELAY;
            }
            // println!("player speed: {}", player.speed);
        }

        if keys_down.len() == 2 && can_move {
            if is_key_down(KeyCode::Right) && is_key_down(KeyCode::Up) {
                player.x += 1;
                player.y -= 1;
                player.last_move_timer = current_time;
                player.speed = BASE_MOVE_DELAY * 2.0;
            }
            if is_key_down(KeyCode::Right) && is_key_down(KeyCode::Down) {
                player.x += 1;
                player.y += 1;
                player.last_move_timer = current_time;
                player.speed = BASE_MOVE_DELAY * 2.0;
            }
            if is_key_down(KeyCode::Left) && is_key_down(KeyCode::Up) {
                player.x -= 1;
                player.y -= 1;
                player.last_move_timer = current_time;
                player.speed = BASE_MOVE_DELAY * 2.0;
            }
            if is_key_down(KeyCode::Left) && is_key_down(KeyCode::Down) {
                player.x -= 1;
                player.y += 1;
                player.last_move_timer = current_time;
                player.speed = BASE_MOVE_DELAY * 2.0;
            }
            // println!("player speed: {}", player.speed);
        }

        next_frame().await
    }
}
