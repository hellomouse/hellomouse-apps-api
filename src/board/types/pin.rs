use uuid::Uuid;
use serde_json::Value;
use serde::{Serialize, Deserialize};
use chrono::Utc;
use num_derive::FromPrimitive;

#[derive(Clone, Serialize, Deserialize)]
#[derive(FromPrimitive)]
pub enum PinType {
    Markdown = 0,
    ImageGallery = 1,
    Link = 2,
    Review = 3
}

// IMPORTANT: Even though it's implemented as a u64
// Do not use more than 32 flags
bitflags::bitflags! {
    #[derive(Serialize, Deserialize, Clone)]
    #[serde(transparent)]
    pub struct PinFlags: u64 {
        const LOCKED = 1 << 0;
        const ARCHIVED = 1 << 1;
        const PINNED = 1 << 2;

        const ALL = 0xFFFFFFFF;
        const NONE = 0b0;
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Pin {
    pub board_id: Uuid,
    pub pin_id: Uuid,
    pub pin_type: PinType,
    pub content: String,
    pub creator: String,
    pub created: chrono::DateTime<Utc>,
    pub edited: chrono::DateTime<Utc>,
    pub flags: PinFlags,
    pub attachment_paths: Vec<String>,
    pub metadata: Value
}

#[derive(Clone, Serialize, Deserialize)]
pub enum SortPin {
    Created,
    Edited
}

impl std::fmt::Display for SortPin {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            SortPin::Created => write!(f, "created"),
            SortPin::Edited => write!(f, "edited"),
        }
    }
}