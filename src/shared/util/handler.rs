//! An abstraction layer interface for API handling
use crate::shared::types::account::{Account, UserId};

use serde_json::Value;
pub trait UserDataHandler {
    // Called on first launch for setup
    fn init(&mut self);

    fn can_login(&self, username: &str, password: &str) -> bool;

    fn create_account(&mut self, user: &UserId, password_hash: &str) -> Result<&Account, &'static str>;
    fn change_account_settings(&mut self, user: &UserId, settings: Value) -> Result<&Account, &'static str>;
    fn delete_account(&mut self, user: &UserId) -> Result<(), &'static str>;

    fn get_user(&self, user_id: &UserId) -> Result<&Account, &'static str>;
}
