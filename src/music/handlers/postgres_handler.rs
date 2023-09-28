use crate::shared::types::account::UserId;
use crate::shared::util::config;
use crate::music::types::{Playlist, PlaylistDetails};

use chrono::Utc;
use uuid::Uuid;
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
        // Stores all music related
        sqlx::query(format!("CREATE SCHEMA IF NOT EXISTS music AUTHORIZATION {};", config::get_config().database.user).as_str())
            .execute(&self.pool).await?;

        sqlx::query(r#"
        CREATE TABLE IF NOT EXISTS music.playlists (
            id uuid primary key unique,
            name text NOT NULL CHECK(length(name) < 128 and length(name) > 0),
            creator_id text NOT NULL REFERENCES users(id),
            song_count INTEGER DEFAULT 0
        );"#).execute(&self.pool).await?;

        // Saves what playlists a user has saved in their list
        sqlx::query(r#"
        CREATE TABLE IF NOT EXISTS music.user_playlists (
            id uuid NOT NULL REFERENCES music.playlists(id),
            user_id text NOT NULL REFERENCES users(id)
        );"#).execute(&self.pool).await?;

        sqlx::query(r#"
        CREATE TABLE IF NOT EXISTS music.songs (
            id uuid primary key unique,
            artist text NOT NULL CHECK(length(artist) < 128 and length(artist) > 0),
            title text NOT NULL CHECK(length(title) < 1024 and length(title) > 0),
            created timestamptz NOT NULL,
            description text NOT NULL CHECK(length(description) < 16192 and length(description) > 0),
            lyrics text NOT NULL CHECK(length(lyrics) < 16192 and length(lyrics) > 0),
            uploader_id text NOT NULL REFERENCES users(id)
        );"#).execute(&self.pool).await?;

        sqlx::query(r#"
        CREATE TABLE IF NOT EXISTS music.playlist_songs (
            playlist_id uuid NOT NULL,
            song_id uuid NOT NULL,
            adder_id text NOT NULL REFERENCES users(id),
            UNIQUE(playlist_id, song_id)
        );"#).execute(&self.pool).await?;
        Ok(())
    }

    pub async fn create_playlist(&self, user_id: &UserId, name: &str) -> Result<Uuid, sqlx::Error> {
        let id = Uuid::new_v4();
        sqlx::query("INSERT INTO music.playlists(id, name, creator_id) VALUES($1, $2, $3) ON CONFLICT DO NOTHING;")
            .bind(id).bind(name).bind(user_id)
            .execute(&self.pool).await?;
        sqlx::query("INSERT INTO music.user_playlists(id, user_id) VALUES($1, $2) ON CONFLICT DO NOTHING;")
            .bind(id).bind(user_id)
            .execute(&self.pool).await?;
        Ok(id)
    }

    pub async fn add_to_user_playlists(&self, user_id: &UserId, id: &Uuid) -> Result<(), sqlx::Error> {
        sqlx::query("INSERT INTO music.user_playlists(id, user_id) VALUES($1, $2) ON CONFLICT DO NOTHING;")
            .bind(id).bind(user_id)
            .execute(&self.pool).await?;
        Ok(())
    }

    pub async fn delete_from_user_playlists(&self, user_id: &UserId, id: &Uuid) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM music.user_playlists WHERE id = $1 AND user_id = $2;")
            .bind(id).bind(user_id)
            .execute(&self.pool).await?;
        Ok(())
    }

    pub async fn edit_playlist(&self, user_id: &UserId, id: &Uuid, name: &str) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE music.playlists SET name = $2 WHERE creator_id = $3 AND id = $1;")
            .bind(id).bind(name).bind(user_id)
            .execute(&self.pool).await?;
        Ok(())
    }

    pub async fn delete_playlist(&self, user_id: &UserId, id: &Uuid) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM music.user_playlists WHERE id = $1;")
            .bind(id).execute(&self.pool).await?;
        sqlx::query("DELETE FROM music.playlists WHERE id = $1 AND creator_id = $2;")
            .bind(id).bind(user_id)
            .execute(&self.pool).await?;
        Ok(())
    }

    pub async fn get_playlist(&self, user_id: &UserId, id: &Uuid)
                -> Result<PlaylistDetails, sqlx::Error> {
        // TODO: get perms
        // TODO: also return whether you have this added
        let row = sqlx::query("SELECT * FROM music.playlists WHERE id = $1 LIMIT 1;")
            .bind(id).fetch_one(&self.pool).await?;
        let is_in_userlist = match sqlx::query("SELECT * FROM music.user_playlists WHERE id = $1 AND user_id = $2 LIMIT 1;")
            .bind(id).bind(user_id).fetch_one(&self.pool).await {
                Ok(_) => true,
                Err(_) => false
            };

        Ok(PlaylistDetails {
            id: row.get::<Uuid, &str>("id"),
            name: row.get::<String, &str>("name"),
            creator_id: row.get::<String, &str>("creator_id"),
            song_count: row.get::<i32, &str>("song_count"),
            is_in_userlist: is_in_userlist
        })
    }

    pub async fn get_playlists(&self, user_id: &UserId)
                -> Result<Vec<Playlist>, sqlx::Error> {
        Ok(sqlx::query("SELECT * FROM music.playlists WHERE id IN (SELECT id from music.user_playlists WHERE user_id = $1) ORDER BY name ASC LIMIT 500;")
            .bind(user_id)
            .map(|row: PgRow| Playlist {
                id: row.get::<Uuid, &str>("id"),
                name: row.get::<String, &str>("name")
            })
            .fetch_all(&self.pool).await?)
    }
}
