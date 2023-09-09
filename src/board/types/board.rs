use crate::shared::types::account;

use chrono::Utc;
use std::collections::HashMap;
use uuid::Uuid;
use serde::{Serialize, Deserialize};

#[derive(Clone, Serialize, Deserialize)]
pub struct Board {
    pub name: String,
    pub id: Uuid,
    pub desc: String,
    pub creator: String,
    pub color: String,
    pub created: chrono::DateTime<Utc>,
    pub edited: chrono::DateTime<Utc>,
    pub perms: HashMap<String, account::Perm>,
    pub pin_count: i32
}

#[derive(Clone, Serialize, Deserialize)]
pub enum SortBoard {
    Name,
    Created,
    Edited
}

impl std::fmt::Display for SortBoard {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            SortBoard::Name => write!(f, "lower(name)"),
            SortBoard::Created => write!(f, "created"),
            SortBoard::Edited => write!(f, "edited"),
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct MassBoardShareUser {
    pub name: String,
    pub perm: account::Perm
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Tag {
    pub name: String,
    pub id: i32,
    pub creator_id: String,
    pub color: String,
    pub board_ids: Vec<Uuid>
}
