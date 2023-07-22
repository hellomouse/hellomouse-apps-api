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
    pub server: ServerConfig
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
    pub port: u16
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
