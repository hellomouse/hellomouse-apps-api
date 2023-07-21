//! An abstraction layer interface for API handling

use crate::shared::types::account::{UserId, Perm};
use crate::board::types::board;
use crate::board::types::pin;

use uuid::Uuid;
use serde_json::Value;


pub trait BoardDataHandler {
    // Called on first launch for setup
    fn init(&mut self);

    fn create_board(&mut self, name: String, creator: &str, desc: String, color: String, perms: Vec<Perm>)
        -> Result<&board::Board, &'static str>;
    fn modify_board(&mut self, board_id: &Uuid, name: Option<String>, desc: Option<String>,
        color: Option<String>, perms: Option<Vec<Perm>>)
            -> Result<&board::Board, &'static str>;
    fn delete_board(&mut self, board_id: &Uuid) -> Result<(), &'static str>;

    fn create_pin(&mut self, creator: &UserId, pin_type: pin::PinType, board_id: &Uuid, content: String,
        attachment_paths: Vec<String>, flags: u32, metadata: Value)
        -> Result<&pin::Pin, &'static str>;
    fn modify_pin(&mut self, pin_id: &Uuid, pin_type: Option<pin::PinType>, board_id: &Option<Uuid>,
        content: Option<String>, attachment_paths: Option<Vec<String>>, flags: Option<u32>, metadata: Option<Value>)
        -> Result<&pin::Pin, &'static str>;
    fn delete_pin(&mut self, pin_id: Uuid) -> Result<(), &'static str>;

    fn get_boards(&self, user: &UserId, offset: Option<u32>, limit: Option<u32>, only_self: Option<bool>, not_self: Option<bool>,
        owner_search: &Option<String>, search_query: &Option<String>)
            -> Result<Vec<&board::Board>,  &'static str>;
    fn get_pins(&self, offset: Option<u32>, limit: Option<u32>, search_query: &Option<String>)
        -> Result<Vec<&pin::Pin>, &'static str>;

    // TODO: get pin search globally
}
