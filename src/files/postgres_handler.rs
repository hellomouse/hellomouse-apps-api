use std::{cmp, error::Error, path::Path};

use crate::shared::util::config::{self};

use actix_multipart::Multipart;
use futures::StreamExt;
use serde::Serialize;
use sqlx::Row;
use sqlx::postgres::PgPool;
use tokio::io::AsyncWriteExt;
use uuid::Uuid;
use std::io::Read;
extern crate sanitize_filename;
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
    id: Uuid,
    file_name: String,
    file_extension: String,
    file_size: i64
}

#[derive(Clone, Serialize)]
pub struct FileUploadResult {
    pub succeeded: Vec<String>,
    pub failed: Vec<u8>
}

impl PostgresHandler {
    pub async fn init(&self) -> Result<(), sqlx::Error> {
        sqlx::query(r#"
        CREATE TABLE IF NOT EXISTS user_files (
            id uuid primary key unique,
            user_id text NOT NULL REFERENCES users(id),
            original_name text NOT NULL CHECK(length(original_name) < 2048),
            file_extension text NOT NULL CHECK(length(file_extension) < 5),
            upload_date timestamp NOT NULL,
            file_size bigint NOT NULL
          );"#).execute(&self.pool).await?;

        tokio::fs::create_dir_all(&self.user_uploads_dir).await.unwrap();
        tokio::fs::create_dir_all(&self.user_uploads_dir_tmp).await.unwrap();

        Ok(())
    }
    pub async fn get_files(&self, user_id: &str, offset: u32, limit: u32) -> Result<Vec<FileResult>, sqlx::Error> {
        let mut files: Vec<FileResult> = Vec::new();
        let mut query = sqlx::query("SELECT id, original_name, file_extension, file_size FROM user_files WHERE user_id = $1 ORDER BY upload_date DESC OFFSET $2 LIMIT $3;")
            .bind(user_id)
            .bind(offset as i32)
            .bind(cmp::min(100, limit as i32))
            .fetch(&self.pool);

        while let Some(row) = query.next().await {
            let row = row?;
            files.push(FileResult {
                id: row.get(0),
                file_name: row.get(1),
                file_extension: row.get(2),
                file_size: row.get(3)
            });
        }

        Ok(files)
    }

    pub async fn file_exists(&self, id: &Uuid) -> Result<String, sqlx::Error> {
        let row = sqlx::query("SELECT user_id, file_extension FROM user_files WHERE id = $1;")
            .bind(id)
            .fetch_one(&self.pool).await?;
        let file_extension = row.get::<String, &str>("file_extension");
        let user_id = row.get::<String, &str>("user_id");

        let file_path = format!("{}/{}/{}.{}", self.user_uploads_dir, user_id, id.to_string(), file_extension);
        Ok(file_path)
    }

    pub fn get_pfp_path(&self, user_id: &String) -> String {
        let user_id = sanitize_filename::sanitize(user_id);
        return format!("{}/{}/{}.png", self.user_uploads_dir, user_id, user_id);
    }

    pub async fn delete_file(&self, user_id: &String, id: &Uuid) -> Result<(), sqlx::Error> {
        let row = sqlx::query("SELECT user_id, file_extension FROM user_files WHERE id = $1 AND user_id = $2;")
            .bind(id).bind(user_id)
            .fetch_one(&self.pool).await?;
        let file_extension = row.get::<String, &str>("file_extension");
        let user_id = row.get::<String, &str>("user_id");
        let file_path = format!("{}/{}/{}.{}", self.user_uploads_dir, user_id, id.to_string(), file_extension);

        tokio::fs::remove_file(file_path).await.unwrap();
        sqlx::query("DELETE FROM user_files WHERE id = $1;")
            .bind(id).execute(&self.pool).await?;

        Ok(())
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
            ($current_path:expr, $async_file:expr, $failed_files:expr, $count:ident, $tx:ident) => {{
                file_cleanup!($current_path, $async_file, $failed_files, $count, $tx);
                $count += 1;
                continue;
             }};
        }

        // Create subdirectory for user files
        tokio::fs::create_dir_all(
            Path::new(&self.user_uploads_dir).join(user_id)
                .to_str().unwrap().to_string()
            ).await.unwrap();

        let mut failed_files: Vec<u8> = Vec::new();
        let mut succeeded_files: Vec<String> = Vec::new();
        let mut count: u8 = 0;

        while let Some(item) = payload.next().await {
            if count == 50 {
                while let Some(_) = payload.next().await {
                    failed_files.push(count)
                }
                return Ok(FileUploadResult { succeeded: succeeded_files, failed: failed_files });
            }

            let mut field = item?;

            let current_path = Path::new(&self.user_uploads_dir_tmp).join(Uuid::new_v4().to_string())
                .to_str().unwrap().to_string();
            let mut async_file = match tokio::fs::File::create(&current_path).await {
                Ok(f) => f,
                Err(_e) => {
                    continue;
                }
            };
    
            let mut file_name = String::new();
            let mut file_extension = String::new();
    
            if let Some(filename) = field.content_disposition().get_filename() {
                let path = Path::new(&filename);
                file_name = path.file_stem().unwrap().to_string_lossy().to_string();
                match path.extension() {
                    Some(val) => { file_extension = val.to_string_lossy().to_string(); }
                    None => {}
                }
            }

            let mut tx = self.pool.begin().await?;
            let mut errored_in_chunk = false;

            while let Some(chunk) = field.next().await {
                match chunk {
                    Ok(chunk) => {
                        match async_file.write_all(&chunk).await {
                            Ok(_) => (),
                            Err(_) => { errored_in_chunk = true; break; }
                        }
                    },
                    Err(_) => { errored_in_chunk = true; break; }
                };
            }
            if errored_in_chunk { file_cleanup_and_continue!(&current_path, async_file, failed_files, count, tx); } // Already processed cleanup
            if file_name.is_empty() && file_extension.is_empty() { file_cleanup_and_continue!(&current_path, async_file, failed_files, count, tx); };
    
            // copy the file to the user's uploads directory with the new filename
            let file_id = Uuid::new_v4();
            let file_path = Path::new(&self.user_uploads_dir)
                .join(user_id)
                .join(format!("{}.{}", file_id.to_string(), file_extension))
                .to_str().unwrap().to_string();

            let file_size: u64;
            let file_metadata = async_file.metadata().await;
            match file_metadata {
                Ok(data) => { file_size = data.len(); },
                Err(_) => { file_cleanup_and_continue!(&current_path, async_file, failed_files, count, tx); }
            };

            if async_file.shutdown().await.is_err() { file_cleanup_and_continue!(&current_path, async_file, failed_files, count, tx); }

            let result = sqlx::query("INSERT INTO user_files (id, user_id, original_name, file_extension, upload_date, file_size) VALUES ($1, $2, $3, $4, $5, $6) ON CONFLICT DO NOTHING;")
                .bind(&file_id)
                .bind(user_id)
                .bind(&file_name)
                .bind(&file_extension)
                .bind(chrono::Utc::now())
                .bind(file_size as i64)
                .execute(&mut *tx).await;
            if result.is_err() { file_cleanup_and_continue!(&current_path, async_file, failed_files, count, tx); }

            if tokio::fs::rename(&current_path, &file_path).await.is_err() { file_cleanup_and_continue!(&current_path, async_file, failed_files, count, tx); }

            tx.commit().await?;
            succeeded_files.push(file_id.to_string());
            count += 1;
        }

        Ok(FileUploadResult { succeeded: succeeded_files, failed: failed_files })
    }

    // For uploading a profile picture
    pub async fn pfp_create(&self, user_id: &str, mut payload: Multipart) -> Result<(), Box<dyn Error>> {
        macro_rules! file_cleanup_and_continue {
            ($current_path:expr, $async_file:expr) => {{
                $async_file.shutdown().await.unwrap();
                tokio::fs::remove_file($current_path).await.unwrap();
                return Err("PFP fail".into());
             }};
        }

        // Create subdirectory for user files
        tokio::fs::create_dir_all(Path::new(&self.user_uploads_dir).join(user_id).to_str().unwrap().to_string()).await.unwrap();

        if let Some(item) = payload.next().await {
            let mut field = item?;
            let mut current_path = Path::new(&self.user_uploads_dir_tmp).join(Uuid::new_v4().to_string())
                .to_str().unwrap().to_string();
            let mut async_file = tokio::fs::File::create(&current_path).await?;
            let mut errored_in_chunk = false;

            while let Some(chunk) = field.next().await {
                match chunk {
                    Ok(chunk) => {
                        match async_file.write_all(&chunk).await {
                            Ok(_) => (),
                            Err(_) => { errored_in_chunk = true; break; }
                        }},
                    Err(_) => { errored_in_chunk = true; break; }
                };
            }
            if errored_in_chunk { file_cleanup_and_continue!(&current_path, async_file); } // Already processed cleanup

            // Validate if file is actually an image (from header)
            let file = std::fs::File::open(&current_path);
            if file.is_err() { file_cleanup_and_continue!(&current_path, async_file); }
            let file = file.unwrap();

            let mut content: Vec<u8> = vec![];
            let actual_ext;
            file.take(12).read_to_end(&mut content)?;

            match imghdr::from_bytes(&content) {
                Some(imghdr::Type::Jpeg) => { actual_ext = "jpg".to_string(); },
                Some(imghdr::Type::Png) => { actual_ext = "png".to_string(); },
                Some(imghdr::Type::Webp) => { actual_ext = "webp".to_string(); },
                _ => { file_cleanup_and_continue!(&current_path, async_file); },
            }

            let new_current_path = current_path.to_string() + "." + actual_ext.as_str();
            if tokio::fs::rename(&current_path, &new_current_path).await.is_err() {
                file_cleanup_and_continue!(&current_path, async_file);
            }
            current_path = new_current_path;

            // Crop the image, convert to png, and upload to user's upload directory
            let file_id = user_id;
            let file_extension = "png";
            let file_path = Path::new(&self.user_uploads_dir)
                .join(user_id)
                .join(format!("{}.{}", file_id.to_string(), file_extension))
                .to_str().unwrap().to_string();

            let img = image::open(&current_path).unwrap();
            if async_file.shutdown().await.is_err() { file_cleanup_and_continue!(&current_path, async_file); }
            let img = img.resize_to_fill(200, 200, image::imageops::FilterType::Triangle);
            img.save(&file_path).unwrap();

            if tokio::fs::remove_file(&current_path).await.is_err() {
                tokio::fs::remove_file(&file_path).await.unwrap();
                file_cleanup_and_continue!(&current_path, async_file);
            }
        }

        Ok(())
    }

}
