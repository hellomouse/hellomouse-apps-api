use crate::shared::types::account;

use uuid::Uuid;
use serde::{Serialize, Deserialize};

#[derive(Clone, Serialize, Deserialize)]
pub struct Board {
    pub name: String,
    pub id: Uuid,
    pub desc: String,
    pub creator: String,
    pub color: String,
    pub created: std::time::SystemTime,
    pub edited: std::time::SystemTime,
    pub perms: Vec<account::Perm>
}
