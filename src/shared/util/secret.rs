use std::fs;
use std::io::prelude::*;
use actix_web::cookie::Key;

pub fn get_session_key() -> std::io::Result<Key> {
    let file_path = "./session-key";
    let contents = fs::read(file_path);

    let master = Key::generate();
    let mut key = Key::derive_from(master.master());
    if contents.is_ok() {
        key = Key::derive_from(&contents.unwrap());
    } else {
        let mut file = fs::File::create(file_path)?;
        file.write_all(master.master())?;
    }

    Ok(key)
}
