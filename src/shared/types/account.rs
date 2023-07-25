//! Account + related schema for database and url parameter validation

use serde::{Serialize, Deserialize};
use serde_json::Value;

pub type UserId = str;

/// A permission level definition for resource editing
/// In general:
/// - **view:** Read only access
/// - **interact:** Only interact with interactable sub-resources, but not create or edit existing sub-resources
/// - **self_edit:** Create, but can only delete / edit sub-resources that were created by themselves
/// - **edit:** Create, and edit/delete anyone's sub-resources
/// - **owner:** Owner of the resource, can delete / edit main resource
#[derive(Clone, Serialize, Deserialize)]
#[derive(sqlx::Type)]
#[repr(i32)]
#[derive(PartialEq)]
pub enum PermLevel {
    View = 0,
    Interact = 1,
    SelfEdit = 2,
    Edit = 3,
    Owner = 4
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Account {
    pub name: String,
    pub id: String,
    pub pfp_url: String,
    pub settings: Value
}

#[derive(Clone, Serialize, Deserialize)]
#[derive(PartialEq)]
pub struct Perm {
    pub perm_level: PermLevel
}
