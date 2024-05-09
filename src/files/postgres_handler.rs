use std::{cmp, error::Error};

use crate::shared::util::config::{self};

use actix_multipart::Multipart;
use futures::StreamExt;
use serde::Serialize;
use sqlx::Row;
use sqlx::postgres::PgPool;
use sha2::{Sha256, Digest};
use tokio::io::AsyncWriteExt;
use uuid::Uuid;

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

#[derive(Clone, Serialize)]
pub struct FileResult {
    file_name: String,
    hash: String,
}

#[derive(Clone, Serialize)]
pub struct FileUploadResult {
    pub succeeded: Vec<String>,
    pub failed: Vec<i8>
}

impl PostgresHandler {
    pub async fn init(&self) -> Result<(), sqlx::Error> {
        sqlx::query(r#"
        CREATE TABLE IF NOT EXISTS user_files (
            file_hash text primary key unique CHECK(
                length(file_hash) = 64 
            ),
            user_id text NOT NULL REFERENCES users(id),
            original_name text NOT NULL CHECK(length(original_name) < 2048),
            file_extension text NOT NULL CHECK(length(file_extension) < 5),
            upload_date timestamp NOT NULL
          );"#).execute(&self.pool).await?;

        tokio::fs::create_dir_all(&self.user_uploads_dir).await.unwrap();
        tokio::fs::create_dir_all(&self.user_uploads_dir_tmp).await.unwrap();

        Ok(())
    }

    pub async fn get_files(&self, user_id: &str, offset: u32, limit: u32) -> Result<Vec<FileResult>, sqlx::Error> {
        let mut files: Vec<FileResult> = Vec::new();
        let mut query = sqlx::query("SELECT original_name, file_hash FROM user_files WHERE user_id = $1 ORDER BY upload_date DESC OFFSET $2 LIMIT $3;")
            .bind(user_id)
            .bind(offset as i32)
            .bind(cmp::min(100, limit as i32))
            .fetch(&self.pool);

        while let Some(row) = query.next().await {
            let row = row?;
            files.push(FileResult {
                file_name: row.get(0),
                hash: row.get(1),
            });
        }

        Ok(files)
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

    /// Create a file in the database and move it to the user's uploads directory
    /// 
    /// # Arguments
    /// 
    /// * `user_id` - The user's id
    /// * `payload` - The file to be uploaded
    /// 
    /// # Returns
    /// 
    /// * `Result` containing a `Vec<i8>` (Indexes of files that failed to upload)
    pub async fn file_create(&self, user_id: &str, mut payload: Multipart) -> Result<FileUploadResult, Box<dyn Error>> {
        macro_rules! file_cleanup {
            ($current_path:expr, $async_file:expr, $failed_files:expr, $count:expr, $tx:ident) => {{
                $async_file.shutdown().await.unwrap();
                tokio::fs::remove_file($current_path).await.unwrap();
                $failed_files.push($count);
                $tx.rollback().await?;
            }};
        }
        macro_rules! file_cleanup_and_continue {
            ($current_path:expr, $async_file:expr, $failed_files:expr, $count:expr, $tx:ident) => {{
                file_cleanup!($current_path, $async_file, $failed_files, $count, $tx);
                continue;
             }};
        }

        let mut failed_files: Vec<i8> = Vec::new();
        let mut succeeded_files: Vec<String> = Vec::new();

        let mut count: i8 = -1;
        while let Some(item) = payload.next().await {
            count += 1;
            let mut field = item?;

            let current_path = format!("{}/{}", self.user_uploads_dir_tmp, Uuid::new_v4().to_string());
            let mut async_file = match tokio::fs::File::create(&current_path).await {
                Ok(f) => f,
                Err(_e) => {
                    continue;
                }
            };
    
            let mut file_name = String::new();
            let mut file_extension = String::new();
    
            if let Some(filename) = field.content_disposition().get_filename() {
                let parts = filename.split_once('.');
                match parts {
                    Some(_) => { // File contained a ., such as test.tar.gz -> test and tar.gz
                        file_name = parts.unwrap().0.to_string();
                        file_extension = parts.unwrap().1.to_string();
                    },
                    None => { // File does not have an extension, ie 'file'
                        file_name = filename.to_string();
                        file_extension = "".to_string();
                    }
                }

                // If the filename is empty and the extension is not, swap them
                if file_name.is_empty() && !file_extension.is_empty() {
                    std::mem::swap(&mut file_name, &mut file_extension);
                    file_name = format!(".{}", file_name);
                }
            }

            let mut tx = self.pool.begin().await?;
            let mut errored_in_chunk = false;

            while let Some(chunk) = field.next().await {
                match chunk {
                    Ok(chunk) => {
                        match async_file.write_all(&chunk).await {
                            Ok(_) => (),
                            Err(_) => {
                                errored_in_chunk = true;
                                break;
                            }
                        }
                    },
                    Err(_) => {
                        errored_in_chunk = true;
                        break;
                    }
                };
            }
            if errored_in_chunk { file_cleanup_and_continue!(&current_path, async_file, failed_files, count, tx); } // Already processed cleanup
            if file_name.is_empty() || file_extension.is_empty() { file_cleanup_and_continue!(&current_path, async_file, failed_files, count, tx); };
    
            // copy the file to the user's uploads directory with the hashed filename
            let file_hash = Self::hash_file_name(&file_name);
            let file_path = format!("{}/{}{}.{}", self.user_uploads_dir, user_id, file_hash, file_extension);
    
            if async_file.shutdown().await.is_err() { file_cleanup_and_continue!(&current_path, async_file, failed_files, count, tx); }

            let result = sqlx::query("INSERT INTO user_files (file_hash, user_id, original_name, file_extension, upload_date) VALUES ($1, $2, $3, $4, $5) ON CONFLICT DO NOTHING;")
                .bind(&file_hash)
                .bind(user_id)
                .bind(&file_name)
                .bind(&file_extension)
                .bind(chrono::Utc::now())
                .execute(&mut *tx).await;
            if result.is_err() { file_cleanup_and_continue!(&current_path, async_file, failed_files, count, tx); }

            if tokio::fs::rename(&current_path, &file_path).await.is_err() { file_cleanup_and_continue!(&current_path, async_file, failed_files, count, tx); }

            tx.commit().await?;
            succeeded_files.push(file_path);
        }

        Ok(FileUploadResult { succeeded: succeeded_files, failed: failed_files })
    }

}
