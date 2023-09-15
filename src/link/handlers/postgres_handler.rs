use crate::shared::types::account::UserId;
use crate::shared::util::config;
use crate::link::types::Link;

use sqlx::Row;
use sqlx::postgres::{PgPool, PgRow};

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
        CREATE TABLE IF NOT EXISTS links (
            id SERIAL PRIMARY KEY,
            url text NOT NULL CHECK(length(url) < 4096),
            creator_id text NOT NULL REFERENCES users(id),
            UNIQUE(creator_id, url)
        );"#).execute(&self.pool).await?;
        Ok(())
    }

    pub async fn add_link(&self, user_id: &UserId, url: &str) -> Result<i32, sqlx::Error> {
        let r = sqlx::query("INSERT INTO links(url, creator_id) VALUES($1, $2) ON CONFLICT DO NOTHING RETURNING id;")
            .bind(url).bind(user_id)
            .fetch_optional(&self.pool).await?;
        Ok(match r {
            None => -1,
            Some(r) => r.get::<i32, &str>("id")
        })
    }

    pub async fn delete_link(&self, user_id: &UserId, id: i32) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM links WHERE id = $1 AND creator_id = $2;")
            .bind(id).bind(user_id)
            .execute(&self.pool).await?;
        Ok(())
    }

    pub async fn get_links(&self, user_id: &UserId)
                -> Result<Vec<Link>, sqlx::Error> {
        Ok(sqlx::query("SELECT * FROM links WHERE creator_id = $1 ORDER BY url DESC LIMIT 500;")
                .bind(user_id)
                .map(|row: PgRow| Link {
                    id: row.get::<i32, &str>("id"),
                    url: row.get::<String, &str>("url"),
                })
                .fetch_all(&self.pool).await?)
    }
}
