/*
// A handler that just stores data in memory (non-persistent)
// Used for debugging purposes
extern crate json_value_merge;

use std::collections::HashMap;
use std::time::SystemTime;
use uuid::Uuid;
use serde_json::{json, Value};
use json_value_merge::Merge;

use crate::board::util::handler::BoardDataHandler;
use crate::shared::types::account::{Account, UserId};
use crate::board::types::pin;
use crate::board::types::board;

macro_rules! update_if_not_none {
    ($base: ident, $property: ident) => {
        if $property.is_some() {
            $base.$property = $property.unwrap();
        }
    };
}

macro_rules! delete_pin_from_board {
    ($self: ident, $pin: ident) => {
        let board = $self.board_pins.get_mut(&$pin.board_id);
        if board.is_some() {
            let board = board.unwrap();
            board.retain(|&x| x != $pin.pin_id);
        }
    }
}

#[derive(Clone)]
pub struct DebugHandler {
    users: HashMap<String, account::Account>,
    boards: HashMap<Uuid, board::Board>,
    pins: HashMap<Uuid, pin::Pin>,
    board_pins: HashMap<Uuid, Vec<Uuid>>
}

impl DebugHandler {
    pub fn new() -> DebugHandler {
        DebugHandler {
            users: HashMap::new(),
            boards: HashMap::new(),
            pins: HashMap::new(),
            board_pins: HashMap::new()
        }
    }
}

impl BoardDataHandler for DebugHandler {
    fn init(&mut self) {
        // Create a dummy user called "admin"
        self.users.insert("admin".to_string(),
            account::Account{
                name: "Admin Amadeus".to_string(),
                id: "admin".to_string(),
                pfp_url: "".to_string(),
                settings: json!("{\"test\":1}")
            });
    }

    fn login(&self, user: &UserId, password: &str) -> bool {
        // All users have the password "password"
        // In a real case you would have real password auth
        // Note: user check should not return early to avoid timing attacks
        match self.users.get(user) {
            None => false,
            _ => password == "password"
        }
    }

    fn create_account(&mut self, user: &UserId, password_hash: &str) -> Result<&account::Account, &'static str> {
        Err("Not implemented")
    }

    fn change_account_settings(&mut self, user: &UserId, settings: Value) -> Result<&account::Account, &'static str> {
        match self.users.get_mut(user) {
            None => Err("change_account_settings: can't find user"),
            Some(usr) => {
                usr.settings.merge(settings);
                Ok(self.users.get(user).unwrap())
            }
        }
    }

    fn delete_account(&mut self, user: &UserId) -> Result<(), &'static str> {
        match self.users.remove(user) {
            None => Err("delete_account: Err, no user by ID"),
            _ => Ok(())
        }
    }

    fn create_board(&mut self, name: String, creator: &UserId, desc: String, color: String, perms: Vec<account::Perm>)
            -> Result<&board::Board, &'static str> {
        let mut id: Uuid;
        loop {
            id = Uuid::new_v4();
            if self.boards.get(&id).is_none() { break; }
        }

        let b = board::Board {
            name,
            id,
            desc,
            creator: creator.to_string(),
            color: color,
            created: SystemTime::now(),
            edited: SystemTime::now(),
            perms
        };
        self.boards.insert(id, b);
        return Ok(self.boards.get(&id).unwrap());
    }

    fn modify_board(&mut self, board_id: &Uuid, name: Option<String>, desc: Option<String>,
            color: Option<String>, perms: Option<Vec<account::Perm>>)
                -> Result<&board::Board, &'static str> {
        let b = self.boards.get_mut(&board_id);
        if b.is_none() {
            return Err("Could not find board id");
        }
        
        let mut b = b.unwrap();
        b.edited = SystemTime::now();

        update_if_not_none!(b, name);
        update_if_not_none!(b, desc);
        update_if_not_none!(b, color);
        update_if_not_none!(b, perms);

        return Ok(self.boards.get(board_id).unwrap());
    }

    fn delete_board(&mut self, board_id: &Uuid) -> Result<(), &'static str> {
        if self.boards.remove(&board_id).is_none() {
            return Err("delete_board: Board ID does not exist");
        }
        
        // Delete associated pins
        let pins = self.board_pins.get_mut(board_id);
        if pins.is_some() {
            let pins = pins.unwrap();
            for pin_id in pins.iter() {
                self.pins.remove(&pin_id);
            }
            pins.clear();
        }

        return Ok(());
    }

    fn create_pin(&mut self, creator: &UserId, pin_type: pin::PinType, board_id: &Uuid, content: String,
            attachment_paths: Vec<String>, flags: u32, metadata: Value) -> Result<&pin::Pin, &'static str> {
        let mut id: Uuid;
        loop {
            id = Uuid::new_v4();
            if self.pins.get(&id).is_none() { break; }
        }

        let p = pin::Pin {
            board_id: *board_id,
            pin_id: id,
            pin_type,
            content,
            creator: creator.to_string(),
            created: SystemTime::now(),
            edited: SystemTime::now(),
            flags,
            attachment_paths,
            metadata
        };
        self.pins.insert(id, p);
        self.board_pins.entry(*board_id).or_insert_with(|| { Vec::new() }).push(id);
        return Ok(self.pins.get(&id).unwrap());
    }

    fn modify_pin(&mut self, pin_id: &Uuid, pin_type: Option<pin::PinType>, board_id: &Option<Uuid>,
            content: Option<String>, attachment_paths: Option<Vec<String>>, flags: Option<u32>, metadata: Option<Value>)
                -> Result<&pin::Pin, &'static str> {
        let p = self.pins.get_mut(&pin_id);
        if p.is_none() {
            return Err("Could not find pin by id");
        }

        let mut p = p.unwrap();

        update_if_not_none!(p, pin_type);
        update_if_not_none!(p, content);
        update_if_not_none!(p, attachment_paths);

        p.flags = flags.unwrap_or(p.flags);
        p.edited = SystemTime::now();

        if metadata.is_some() {
            p.metadata.merge(metadata.unwrap());
        }

        // Board id needs updating
        if board_id.is_some() && board_id.unwrap() != p.board_id {
            delete_pin_from_board!(self, p);
            self.board_pins.entry(board_id.unwrap()).or_insert_with(|| { Vec::new() }).push(p.pin_id);
            p.pin_id = board_id.unwrap();
        }

        return Ok(self.pins.get(pin_id).unwrap());
    }

    fn delete_pin(&mut self, pin_id: Uuid) -> Result<(), &'static str> {
        let pin = self.pins.remove(&pin_id);
        if pin.is_some() {
            let pin = &pin.unwrap();
            delete_pin_from_board!(self, pin);
            return Ok(());
        }
        return Err("delete_pin: Pin ID does not exist");
    }

    fn get_boards(&self, user: &UserId, offset: Option<u32>, limit: Option<u32>, only_self: Option<bool>, not_self: Option<bool>,
            owner_search: &Option<String>, search_query: &Option<String>)
                -> Result<Vec<&board::Board>,  &'static str> {
        let mut i = 0;
        let mut result: Vec<&board::Board> = Vec::new();

        for (key, value) in &self.boards {
            i += 1;
            if i >= offset.unwrap_or(0) {
                // TODO: use iterator or something
                result.push(self.boards.get(key).unwrap());
            }
            if result.len() >= limit.unwrap_or(20) as usize {
                break;
            }
        }
        return Ok(result);
    }

    fn get_pins(&self, offset: Option<u32>, limit: Option<u32>, search_query: &Option<String>)
            -> Result<Vec<&pin::Pin>, &'static str> {
        let result: Vec<&pin::Pin> = Vec::new();
        // TODO
        return Ok(result);
    }

    fn get_user(&self, user_id: &UserId) -> Result<&account::Account, &'static str> {
        match self.users.get(user_id) {
            None => Err("get_user: User id does not exist"),
            Some(val) => Ok(val)
        }
    }
}
 */