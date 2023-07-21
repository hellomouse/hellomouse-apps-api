use uuid::Uuid;
use bitflags::bitflags;
use serde_json::Value;
use serde::{Serialize, Deserialize};

#[derive(Clone, Serialize, Deserialize)]
pub enum PinType {
    Markdown = 0,
    ImageGallery = 1,
    Link = 2,
    Review = 3
}

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
    pub created: std::time::SystemTime,
    pub edited: std::time::SystemTime,
    pub flags: PinFlags,
    pub attachment_paths: Vec<String>,
    pub metadata: Value
}
