use crate::shared::types::account::UserId;
use crate::shared::util::config;
use crate::site::types::status::Job;

use uuid::Uuid;
use sqlx::Row;
use sqlx::postgres::{PgPool, PgRow};
use std::cmp;
use chrono::Utc;

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
            name text NOT NULL CHECK(length(data) < 512),
            data text NOT NULL CHECK(length(data) < 4096),
            requestor text NOT NULL,
            priority integer NOT NULL
        );"#).execute(&self.pool).await?;

        // Create site completed work table
        sqlx::query(r#"
        CREATE TABLE IF NOT EXISTS site.status (
            id uuid primary key unique,
            created timestamptz NOT NULL,
            finished timestamptz NOT NULL,
            name text NOT NULL CHECK(length(name) < 512),
            data text NOT NULL CHECK(length(data) < 4096),
            requestor text NOT NULL,
            priority integer NOT NULL,
            status text NOT NULL CHECK(length(status) < 512)
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
        sqlx::query("INSERT INTO site.status VALUES($1, $2, $3, $4, $5, $6, $7, $8);")
            .bind(uuid).bind(now).bind(now).bind(cmd).bind(data)
            .bind(user.to_string()).bind(priority).bind("queued")
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

    pub async fn get_status_queue(&self, user: &UserId, offset: Option<u32>, limit: Option<u32>)
                -> Result<Vec<Job>, sqlx::Error> {
        Ok(sqlx::query("SELECT * FROM site.status WHERE
                    requestor = $1
                    ORDER BY created DESC
                    OFFSET $2 LIMIT $3;")
                .bind(user)
                .bind(offset.unwrap_or(0) as i32)
                .bind(cmp::min(100, limit.unwrap_or(20) as i32))
                .map(|row: PgRow| Job {
                    id: row.get::<Uuid, &str>("id"),
                    created: row.get::<chrono::DateTime<Utc>, &str>("created"),
                    finished: row.get::<chrono::DateTime<Utc>, &str>("finished"),
                    name: row.get::<String, &str>("name"),
                    data: row.get::<String, &str>("data"),
                    requestor: row.get::<String, &str>("requestor"),
                    priority: row.get::<i32, &str>("priority"),
                    status: row.get::<String, &str>("status"),
                })
                .fetch_all(&self.pool).await?)
    }
}
