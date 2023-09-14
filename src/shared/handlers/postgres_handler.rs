use crate::shared::util::config;
use crate::shared::types::account::{Account, UserId};

use serde_json::Value;
use serde::Serialize;
use json_value_merge::Merge;
use sqlx::Row;
use sqlx::postgres::{PgRow, PgPool};

#[derive(Clone, Serialize)]
pub struct UserSearchResult {
    name: String,
    id: String,
    pfp_url: String
}

#[derive(Clone)]
pub struct PostgresHandler {
    pool: PgPool
}

impl PostgresHandler {
    pub async fn new() -> Result<PostgresHandler, sqlx::Error> {
        Ok(PostgresHandler { pool: config::get_pool().await })
    }
}

impl PostgresHandler {
    // Called on first launch for setup
    pub async fn init(&self) -> Result<(), sqlx::Error> {
        sqlx::query(r#"
        CREATE TABLE IF NOT EXISTS users (
            id text primary key unique CHECK(length(id) < 25 and id ~ '^[a-zA-Z0-9_]+$'),
            name text NOT NULL CHECK(length(name) < 45 and name ~ '^[a-zA-Z0-9_]+$'),
            pfp_url text CHECK(length(pfp_url) < 2048),
            settings json CHECK(pg_column_size(settings) < 1048576),
            password_hash text NOT NULL
        );"#).execute(&self.pool).await?;

        // Create a dummy public user_id
        match self.create_account("Public", "public", "this_password_doesnt_matter").await {
            Ok(_) => (),
            Err(_) => ()
        };
        Ok(())
    }

    pub async fn can_login(&self, user_id: &UserId, mut password: &str) -> Result<bool, sqlx::Error> {
        if user_id == "public" { return Ok(false); } // Cannot log into public user_id

        // Too long password: replace password with a dummy and flag
        // that it should always be invalid
        let mut password_correct_override = true;
        if password.len() > config::get_config().count.max_password_length {
            password = "fake_password";
            password_correct_override = false;
        }

        let p = match sqlx::query("SELECT * FROM users WHERE id = $1;")
            .bind(user_id).fetch_one(&self.pool).await {
            Ok(user_id) => user_id.get::<String, &str>("password_hash"),
            Err(_err) => "".to_string()
        };
        Ok(libpasta::verify_password(&p, &password) && password.chars().count() > 0 && password_correct_override)
    }

    pub async fn create_account(&self, user_id: &UserId, name: &str, password: &str) -> Result<(), sqlx::Error> {
        let password_hash = libpasta::hash_password(&password);
        sqlx::query("INSERT INTO users(id, name, password_hash) VALUES($1, $2, $3);")
            .bind(user_id.to_lowercase()).bind(name).bind(password_hash)
            .execute(&self.pool).await?;
        Ok(())
    }

    pub async fn change_account_settings(&self, user_id: &UserId, settings: Value) -> Result<(), sqlx::Error> {
        let user = sqlx::query("SELECT * FROM users where id = $1;")
            .bind(user_id).fetch_one(&self.pool).await?;
        let mut new_settings: Value = user.try_get::<Value, &str>("settings").unwrap_or(
            serde_json::from_str("{}").unwrap());
    
        new_settings.merge(settings);

        sqlx::query("UPDATE users SET settings = to_json($1) WHERE id = $2;")
            .bind(new_settings).bind(user.get::<String, &str>("id"))
            .execute(&self.pool).await?;
        Ok(())
    }

    pub async fn change_password(&self, user_id: &UserId, password: &str) -> Result<(), sqlx::Error> {
        let password_hash = libpasta::hash_password(&password);
        sqlx::query("UPDATE users SET password_hash = $1 WHERE id = $2;")
            .bind(password_hash).bind(user_id)
            .execute(&self.pool).await?;
        Ok(())
    }

    pub async fn search_users(&self, filter: &str) -> Result<Vec<UserSearchResult>, sqlx::Error> {
        Ok(sqlx::query("SELECT * FROM users WHERE ((id ILIKE $1 || '%') or 
            (name ILIKE '%' || $1 || '%')) and id != 'public' LIMIT 20;")
                .bind(filter)
                .map(|row: PgRow| UserSearchResult {
                    name: row.get::<String, &str>("name"),
                    id: row.get::<String, &str>("id"),
                    pfp_url: row.get::<Option<String>, &str>("pfp_url").unwrap_or("".to_string())
                })
                .fetch_all(&self.pool).await?)
    }

    pub async fn get_users_batch(&self, ids: &Vec<String>) -> Result<Vec<UserSearchResult>, sqlx::Error> {
        Ok(sqlx::query("SELECT * FROM users WHERE id = ANY($1) and id != 'public' LIMIT 20;")
                .bind(ids)
                .map(|row: PgRow| UserSearchResult {
                    name: row.get::<String, &str>("name"),
                    id: row.get::<String, &str>("id"),
                    pfp_url: row.get::<Option<String>, &str>("pfp_url").unwrap_or("".to_string())
                })
                .fetch_all(&self.pool).await?)
    }

    pub async fn delete_account(&self, user_id: &UserId) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM users WHERE id = $1;")
            .bind(user_id).execute(&self.pool).await?;
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
