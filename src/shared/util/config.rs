//! Loads config from config.toml

use serde_derive::Deserialize;
use cached::proc_macro::cached;
use std::fs;
use std::process::exit;
use toml;
use sqlx::postgres::{PgPoolOptions, PgPool};

#[derive(Deserialize, Clone)]
pub struct Config {
    pub database: DatabaseConfig,
    pub server: ServerConfig,
    pub count: CountConfig,
    pub music: MusicConfig
}

#[derive(Deserialize, Clone)]
pub struct CountConfig {
    pub min_password_length: usize,
    pub max_password_length: usize
}

#[derive(Deserialize, Clone)]
pub struct DatabaseConfig {
    pub ip: String,
    pub port: u16,
    pub user: String,
    pub password: String,
    pub name: String
}

#[derive(Deserialize, Clone)]
pub struct ServerConfig {
    pub port: u16,
    pub log: bool,
    pub login_cookie_valid_duration_seconds: u64,

    pub user_uploads_dir: String,
    pub user_uploads_dir_tmp: String,
    pub request_quota_replenish_ms: u64,
    pub request_quota: u32,
    pub login_attempt_window: String,
    pub login_attempt_max_per_window: u32
}

#[derive(Deserialize, Clone)]
pub struct MusicConfig {
    pub max_songs_in_queue: u64 // Should match that of the UI
}

#[cached]
pub async fn get_pool() -> PgPool {
    let config = get_config();
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(format!("postgres://{}:{}@{}:{}/{}", // user:password / ip/db
            config.database.user,
            config.database.password,
            config.database.ip,
            config.database.port,
            config.database.name
        ).as_str())
        .await;
    pool.unwrap()
}

#[cached]
pub fn get_config() -> Config {
    let contents = match fs::read_to_string("config.toml") {
        Ok(c) => c,
        Err(_) => {
            eprintln!("Could not find config.toml, please create or ensure it's accessible");
            exit(1);
        }
    };

    let data: Config = match toml::from_str(&contents) {
        Ok(d) => d,
        Err(_) => {
            eprintln!("Unable to load data from config");
            exit(1);
        }
    };
    return data;
}
