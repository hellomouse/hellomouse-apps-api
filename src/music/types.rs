use uuid::Uuid;

use serde::{Serialize, Deserialize};

#[derive(Clone, Serialize, Deserialize)]
pub struct Playlist {
    pub name: String,
    pub id: Uuid
}

#[derive(Clone, Serialize, Deserialize)]
pub struct PlaylistDetails {
    pub name: String,
    pub id: Uuid,
    pub creator_id: String,
    pub song_count: i32,
    pub is_in_userlist: bool
}

