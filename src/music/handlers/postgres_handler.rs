use crate::shared::types::account::{UserId, Perm, PermLevel};
use crate::shared::util::config;
use crate::music::types::{Playlist, PlaylistDetails};

use std::collections::HashMap;
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

        sqlx::query(r#"CREATE TABLE IF NOT EXISTS music.playlist_perms (
            id uuid NOT NULL REFERENCES music.playlists(id),
            perm_id integer,
            user_id text NOT NULL REFERENCES users(id),
            UNIQUE(id, user_id)
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
        sqlx::query(r#"INSERT INTO music.playlist_perms(id, user_id, perm_id) VALUES($1, $2, $3);"#)
            .bind(id).bind(user_id).bind(PermLevel::Owner)
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

    pub async fn edit_playlist(&self, id: &Uuid, name: &str) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE music.playlists SET name = $2 WHERE id = $1;")
            .bind(id).bind(name)
            .execute(&self.pool).await?;
        Ok(())
    }

    pub async fn edit_playlist_perms(&self, user_id: &UserId, id: &Uuid, mut perms: HashMap<String, Perm>) -> Result<(), sqlx::Error> {
        let p = self.get_playlist(&user_id, &id).await.unwrap();
        let mut tx = self.pool.begin().await?;

        if p.perms.get(&user_id as &str).is_some() && p.perms.get(&user_id as &str).unwrap().perm_level == PermLevel::Edit {
            // Cannot make anyone else owner
            let mut bad_keys: Vec<String> = Vec::new();
            for (user, perm) in &mut perms {
                if perm.perm_level == PermLevel::Owner {
                    bad_keys.push(user.clone().to_string());
                }
            }
            for user in bad_keys {
                let mut perm = perms.get(&user).unwrap().clone();
                perm.perm_level = PermLevel::Edit;
                perms.insert(user, perm);
            }
            // Editors cannot lower the permissions of other editors / owners
            // (Other than themselves)
            for (user, perm) in p.perms {
                if (perm.perm_level == PermLevel::Edit || perm.perm_level == PermLevel::Owner) &&
                        user != user_id {
                    perms.insert(user, perm.clone());
                }
            }
        }

        // Delete all existing perms, then insert new perms
        sqlx::query(r#"DELETE FROM music.playlist_perms WHERE id = $1;"#)
            .bind(id).execute(&mut *tx).await?;

        // Creator gets owner permission by default
        sqlx::query(r#"INSERT INTO music.playlist_perms(id, user_id, perm_id) VALUES($1, $2, $3);"#)
            .bind(id).bind(p.creator_id.clone()).bind(PermLevel::Owner)
            .execute(&mut *tx).await?;

        for (perm_user_id, val) in perms {
            // Ignore playlist creator: Always owner permission as defined above
            if perm_user_id == p.creator_id {
                continue;
            }

            // Ignore users that don't exist
            if sqlx::query(r#"SELECT * FROM users where id = $1;"#)
                    .bind(perm_user_id.clone()).fetch_one(&self.pool).await
                    .is_err() {
                continue;
            }

            let result = sqlx::query(r#"INSERT INTO music.playlist_perms(id, user_id, perm_id) VALUES($1, $2, $3);"#)
                .bind(id).bind(perm_user_id.clone()).bind(val.perm_level.clone())
                .execute(&mut *tx).await;
            if result.is_err() {
                tx.rollback().await?;
                return Err(result.unwrap_err());
            }
        }
        tx.commit().await?;
        Ok(())
    }

    pub async fn delete_playlist(&self, id: &Uuid) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM music.user_playlists WHERE id = $1;")
            .bind(id).execute(&self.pool).await?;
        sqlx::query("DELETE FROM music.playlists WHERE id = $1;")
            .bind(id).execute(&self.pool).await?;
        Ok(())
    }

    async fn get_user_perm(&self, user_id: &UserId, id: &Uuid) -> Result<PermLevel, sqlx::Error> {
        let perms = sqlx::query("SELECT * FROM music.playlist_perms WHERE id = $1 AND user_id = $2")
            .bind(id).bind(user_id)
            .fetch_one(&self.pool).await?;
        Ok(perms.get("perm_id"))
    }

    pub async fn can_user_view_playlist(&self, user_id: &UserId, id: &Uuid) -> Result<bool, sqlx::Error> {
        let perm = self.get_user_perm(user_id, id).await?;
        Ok(perm == PermLevel::View || perm == PermLevel::Edit || perm == PermLevel::Owner)
    }

    pub async fn can_user_edit_playlist(&self, user_id: &UserId, id: &Uuid) -> Result<bool, sqlx::Error> {
        let perm = self.get_user_perm(user_id, id).await?;
        Ok(perm == PermLevel::Edit || perm == PermLevel::Owner)
    }

    pub async fn is_user_owner_playlist(&self, user_id: &UserId, id: &Uuid) -> Result<bool, sqlx::Error> {
        let perm = self.get_user_perm(user_id, id).await?;
        Ok(perm == PermLevel::Owner)
    }

    pub async fn get_playlist(&self, user_id: &UserId, id: &Uuid)
                -> Result<PlaylistDetails, sqlx::Error> {
        let row = sqlx::query("SELECT * FROM music.playlists WHERE id = $1 LIMIT 1;")
            .bind(id).fetch_one(&self.pool).await?;
        let is_in_userlist = match sqlx::query("SELECT * FROM music.user_playlists WHERE id = $1 AND user_id = $2 LIMIT 1;")
            .bind(id).bind(user_id).fetch_one(&self.pool).await {
                Ok(_) => true,
                Err(_) => false
            };
        let perms: HashMap<String, Perm> = sqlx::query("SELECT * FROM music.playlist_perms WHERE id = $1")
            .bind(id).map(|row: PgRow| (
                row.get("user_id"),
                Perm { perm_level: row.get("perm_id") }
            ))
            .fetch_all(&self.pool).await.unwrap_or(Vec::new()).into_iter().collect();

        Ok(PlaylistDetails {
            id: row.get::<Uuid, &str>("id"),
            name: row.get::<String, &str>("name"),
            creator_id: row.get::<String, &str>("creator_id"),
            song_count: row.get::<i32, &str>("song_count"),
            is_in_userlist: is_in_userlist,
            perms
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
