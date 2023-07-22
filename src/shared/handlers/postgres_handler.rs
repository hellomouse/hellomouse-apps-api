use crate::shared::util::config;
use crate::shared::types::account::{Account, UserId};

use serde_json::Value;
use json_value_merge::Merge;
use sqlx::Row;
use sqlx::postgres::{PgPoolOptions, PgPool};

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
            id text primary key unique CHECK(length(id) < 25),
            name text NOT NULL CHECK(length(name) < 45),
            pfp_url text CHECK(length(pfp_url) < 2048),
            settings json CHECK(pg_column_size(settings) < 1048576),
            password_hash text NOT NULL
        );"#,
            ).execute(&self.pool).await?;
        Ok(())
    }

    pub async fn can_login(&self, user: &str, password: &str) -> Result<bool, sqlx::Error> {
        let p = match sqlx::query("SELECT * FROM users WHERE id = $1;")
            .bind(user).fetch_one(&self.pool).await {
            Ok(user) => user.get::<String, &str>("password_hash"),
            Err(err) => "".to_string()
        };
        Ok(libpasta::verify_password(&p, &password) && password.chars().count() > 0)
    }

    pub async fn create_account(&mut self, user: &UserId, username: &str, password: &str) -> Result<(), sqlx::Error> {
        let password_hash = libpasta::hash_password(&password);
        sqlx::query("INSERT INTO users(id, name, password_hash) VALUES($1, $2, $3);")
            .bind(user).bind(username).bind(password_hash)
            .execute(&self.pool).await?;
        Ok(())
    }

    pub async fn change_account_settings(&mut self, user: &UserId, settings: Value) -> Result<(), sqlx::Error> {
        let user = sqlx::query("SELECT * FROM users where id = $1;")
            .bind(user).fetch_one(&self.pool).await?;
        let mut new_settings: Value = user.try_get::<Value, &str>("settings").unwrap_or(
            serde_json::from_str("{}").unwrap());
    
        new_settings.merge(settings);

        sqlx::query("UPDATE users SET settings = to_json($1) WHERE id = $2;")
            .bind(new_settings).bind(user.get::<String, &str>("id"))
            .execute(&self.pool).await?;
        Ok(())
    }

    // TODO: update pfp pic

    pub async fn delete_account(&mut self, user: &UserId) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM users WHERE id = $1;")
            .bind(user).execute(&self.pool).await?;
        Ok(())
    }

    pub async fn get_user(&self, user_id: &UserId) -> Result<Account, sqlx::Error> {
        let user = sqlx::query("SELECT * FROM users WHERE id = $1;")
            .bind(user_id).fetch_one(&self.pool).await?;
        let acc = Account{
            name: user.get::<String, &str>("name"),
            id: user.get::<String, &str>("id"),
            pfp_url: user.try_get::<String, &str>("pfp_url").unwrap_or("".to_string()),
            settings: user.try_get::<Value, &str>("settings").unwrap_or(
                serde_json::from_str("{}").unwrap())
        };
        Ok(acc)
    }
}
