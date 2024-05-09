use serde::{Serialize, Deserialize};

#[derive(Clone, Serialize, Deserialize)]
pub struct Link {
    pub url: String,
    pub id: i32
}
