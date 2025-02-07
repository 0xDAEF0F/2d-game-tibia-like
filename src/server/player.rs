// use serde::{Deserialize, Serialize};

#[derive(Debug)]
pub struct Player {
    pub id: u32,
    pub last_request_from_client: u32,
    pub location: (u32, u32),
}
