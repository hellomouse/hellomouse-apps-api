use crate::shared::types::account::UserId;
use crate::shared::util::config;

use serde_json::Value;
use serde::Serialize;

use uuid::Uuid;
use sqlx::postgres::PgPool;

#[derive(Clone)]
pub struct WebHandler {
    pool: PgPool
}

impl WebHandler {
    pub async fn new() -> Result<WebHandler, sqlx::Error> {
        Ok(WebHandler { pool: config::get_pool().await })
    }
}

// TODO: move to types
#[derive(Serialize)]
pub struct SitePreviewReturn {
    desc: String,
    title: String,
    name: String,
    image: String,
    video: String
}

impl WebHandler {
    // Called on first launch for setup
    pub async fn init(&self) -> Result<(), sqlx::Error> {
        sqlx::query(format!("CREATE SCHEMA IF NOT EXISTS site AUTHORIZATION {};", config::get_config().database.user).as_str())
            .execute(&self.pool).await?;

        // Create site work queue db
        sqlx::query(r#"
        CREATE TABLE IF NOT EXISTS site.queue (
            id uuid primary key unique,
            created timestamptz NOT NULL,
            name text NOT NULL,
            data text NOT NULL,
            requestor text NOT NULL
        );"#).execute(&self.pool).await?;
        Ok(())
    }

    pub async fn get_preview(&self, pin_id: &Uuid, url: &str, user: &UserId) -> Result<SitePreviewReturn, sqlx::Error> {
        let now = chrono::offset::Utc::now();
        let uuid = Uuid::new_v4();

        // TODO: make method
        sqlx::query("INSERT INTO site.queue VALUES($1, $2, $3, $4, $5);")
            .bind(uuid).bind(now).bind("pin_preview").bind(format!("{}|{}", pin_id, url)).bind(user.to_string())
            .execute(&self.pool).await?;
        sqlx::query("NOTIFY hellomouse_apps_site_update;")
            .execute(&self.pool).await?;

        Ok(SitePreviewReturn {
            title: "TODO".to_string(),
            desc: "TODO".to_string(),
            name: "TODO".to_string(),
            image: "TODO".to_string(),
            video: "TODO".to_string()
        })
    }
}
