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
