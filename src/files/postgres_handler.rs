use crate::shared::util::config::{self, Config};
use crate::shared::types::account::{Account, UserId};

use serde_json::Value;
use serde::Serialize;
use json_value_merge::Merge;
use sqlx::Row;
use sqlx::postgres::{PgRow, PgPool};
use sha2::{Sha256, Digest};

#[derive(Clone, Serialize)]
pub struct UserSearchResult {
    name: String,
    id: String,
    pfp_url: String
}

#[derive(Clone)]
pub struct PostgresHandler {
    pool: PgPool,
    user_uploads_dir: String,
    user_uploads_dir_tmp: String
}

impl PostgresHandler {
    pub async fn new() -> Result<PostgresHandler, sqlx::Error> {
        let config = config::get_config();
        Ok(PostgresHandler { pool: config::get_pool().await, user_uploads_dir: config.server.user_uploads_dir, user_uploads_dir_tmp: config.server.user_uploads_dir_tmp })
    }
}

impl PostgresHandler {
    pub async fn init(&self) -> Result<(), sqlx::Error> {
        sqlx::query(r#"
        CREATE TABLE IF NOT EXISTS user_files (
            file_hash text primary key unique CHECK(
              length(file_hash) = 64 
            ),
            user_id text NOT NULL CHECK(
              length(user_id) < 25 and user_id ~ '^[a-zA-Z0-9_]+$'
            ),
            original_name text NOT NULL CHECK(length(original_name) < 2048),
            file_extension text NOT NULL CHECK(length(file_extension) < 5),
            upload_date timestamp NOT NULL
          );"#).execute(&self.pool).await?;

        tokio::fs::create_dir_all(&self.user_uploads_dir).await.unwrap();

        Ok(())
    }

    fn hash_file_name(file_name: &str) -> String {
        format!("{:x}", Sha256::digest(file_name.as_bytes()))
    }

    pub async fn file_exists(&self, user_id: &str, file_hash: &str) -> Result<String, sqlx::Error> {
        let file_extension = sqlx::query("SELECT file_extension FROM user_files WHERE file_hash = $1 AND user_id = $2;")
            .bind(file_hash)
            .bind(user_id)
            .fetch_one(&self.pool).await?
            .get::<String, &str>("file_extension");

        let file_path = format!("{}/{}{}.{}", self.user_uploads_dir, user_id, file_hash, file_extension);

        Ok(file_path)
    }

}
