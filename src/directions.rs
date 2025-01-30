#[derive(Debug)]
pub enum Directions {
    North,
    East,
    South,
    West,
    NorthEast,
    NorthWest,
    SouthEast,
    SouthWest,
}

impl From<Directions> for (i8, i8) {
    fn from(direction: Directions) -> Self {
        match direction {
            Directions::North => (0, -1),
            Directions::East => (1, 0),
            Directions::South => (0, 1),
            Directions::West => (-1, 0),
            Directions::NorthEast => (1, -1),
            Directions::NorthWest => (-1, -1),
            Directions::SouthEast => (1, 1),
            Directions::SouthWest => (-1, 1),
        }
    }
}
