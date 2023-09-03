use crate::shared::types::account::UserId;
use crate::shared::util::config;

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
            requestor text NOT NULL,
            priority integer NOT NULL
        );"#).execute(&self.pool).await?;
        Ok(())
    }

    pub async fn queue_task(&self, cmd: &str, data: &str, user: &UserId, priority: i32) -> Result<Uuid, sqlx::Error> {
        let now = chrono::offset::Utc::now();
        let uuid = Uuid::new_v4();

        sqlx::query("INSERT INTO site.queue VALUES($1, $2, $3, $4, $5, $6);")
            .bind(uuid).bind(now).bind(cmd).bind(data)
            .bind(user.to_string()).bind(priority)
            .execute(&self.pool).await?;
        sqlx::query("NOTIFY hellomouse_apps_site_update;")
            .execute(&self.pool).await?;

        Ok(uuid)
    }

    pub async fn get_preview(&self, pin_id: &Uuid, url: &str, user: &UserId) -> Result<(), sqlx::Error> {
        self.queue_task("pin_preview", format!("{}|{}", pin_id, url).as_str(), user, 10).await?;
        Ok(())
    }

    pub async fn queue_site_download(&self, strategy: &str, url: &str, user: &UserId) -> Result<Uuid, sqlx::Error> {
        Ok(self.queue_task(strategy, url, user, 0).await?)
    }
}
