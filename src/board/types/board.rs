use crate::shared::types::account;

use chrono::Utc;
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
    pub perms: Vec<account::Perm>
}
