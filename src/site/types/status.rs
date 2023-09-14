use serde::{Serialize, Deserialize};
use uuid::Uuid;
use chrono::Utc;

#[derive(Clone, Serialize, Deserialize)]
pub struct Job {
    pub id: Uuid,
    pub created: chrono::DateTime<Utc>,
    pub finished: chrono::DateTime<Utc>,
    pub name: String,
    pub data: String,
    pub requestor: String,
    pub priority: i32,
    pub status: String
}
