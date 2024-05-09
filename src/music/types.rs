use crate::shared::types::account::Perm;

use chrono;
use chrono::Utc;
use uuid::Uuid;
use std::collections::HashMap;
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
    pub is_in_userlist: bool,
    pub perms: HashMap<String, Perm>
}

#[derive(Clone, Serialize, Deserialize)]
pub struct SongAbridged {
    pub id: String,
    pub uploader: String,
    pub title: String,
    pub duration_string: String,
    pub thumbnail_file: String
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Song {
    pub uploader: String,
    pub uploader_url: String,
    pub upload_date: chrono::DateTime<Utc>,
    pub title: String,
    pub duration_string: String,
    pub description: String,
    pub thumbnail_file: String,
    pub video_file: String,
    pub subtitle_files: Vec<String>
}
