use crate::shared::util::handler::UserDataHandler;
use crate::shared::util::config;
use crate::shared::types::account::{Account, UserId};

use serde_json::Value;
use sqlx::Row;
use sqlx::postgres::{PgPoolOptions, PgPool, PgRow};

pub struct PostgresHandler {
    pool: PgPool
}

impl PostgresHandler {
    pub async fn new() -> Result<PostgresHandler, sqlx::Error> {
        let config = config::get_config();
        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect(format!("postgres://{}:{}@{}:{}/{}", // user:password / ip/db
                config.database.user,
                config.database.password,
                config.database.ip,
                config.database.port,
                config.database.name
            ).as_str())
            .await?;
        Ok(PostgresHandler { pool })
    }
}

impl PostgresHandler {
    // Called on first launch for setup
    pub async fn init(&mut self) -> Result<(), sqlx::Error> {
        sqlx::query(r#"
        CREATE TABLE IF NOT EXISTS users (
            id text primary key unique,
            name text not null,
            pfp_url text,
            settings json,
            password_hash text not null
        );"#,
            ).execute(&self.pool).await?;
        Ok(())
    }

    pub async fn can_login(&self, user: &str, password: &str) -> Result<bool, sqlx::Error> {
        let p = match sqlx::query("SELECT * FROM users where id = $1;")
            .bind(user).fetch_one(&self.pool).await {
            Ok(user) => user.get::<String, &str>("password_hash"),
            Err(err) => "".to_string()
        };
        Ok(p == password && password.chars().count() > 0)
    }

    pub async fn create_account(&mut self, user: &UserId, username: &str, password_hash: &str) -> Result<(), sqlx::Error> {
        sqlx::query("INSERT into users(id, name, password_hash) values($1, $2, $3);")
            .bind(user).bind(username).bind(password_hash)
            .execute(&self.pool).await?;
        Ok(())
    }

    pub fn change_account_settings(&mut self, user: &UserId, settings: Value) -> Result<&Account, &'static str> {
        Err("Bad")
    }
    pub fn delete_account(&mut self, user: &UserId) -> Result<(), &'static str> {
        Err("Bad")
    }

    pub fn get_user(&self, user_id: &UserId) -> Result<&Account, &'static str> {
        Err("Bad")
    }
}
