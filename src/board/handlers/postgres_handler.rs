use crate::shared::types::account::{UserId, Perm, PermLevel};
use crate::board::types::pin::{self, PinFlags};
use crate::board::types::board;
use crate::shared::util::config;
use crate::shared::util::clean_html::clean_html;

use actix_web::cookie::time::Duration;
use chrono;
use chrono::Utc;
use uuid::{Uuid, uuid};
use std::collections::{HashMap, HashSet};
use std::cmp;
use serde_json::Value;
use futures::StreamExt;

use num;

use sqlx::Row;
use sqlx::postgres::{PgPool, PgRow};

macro_rules! update_if_not_none {
    ($base: ident, $property: ident) => {
        if $property.is_some() {
            $base.$property = $property.unwrap();
        }
    };
}

#[derive(Clone)]
pub struct PostgresHandler {
    pool: PgPool
}

impl PostgresHandler {
    pub async fn new() -> Result<PostgresHandler, sqlx::Error> {
        Ok(PostgresHandler { pool: config::get_pool().await })
    }

    // Called on first launch for setup
    pub async fn init(&self) -> Result<(), sqlx::Error> {
        // Stores all perms
        sqlx::query(format!("CREATE SCHEMA IF NOT EXISTS board AUTHORIZATION {};", config::get_config().database.user).as_str())
            .execute(&self.pool).await?;

        sqlx::query(r#"CREATE TABLE IF NOT EXISTS board.boards (
            id uuid primary key unique,
            name text NOT NULL CHECK(length(name) < 4096),
            description text NOT NULL,
            creator_id text NOT NULL REFERENCES users(id),
            color text NOT NULL CHECK(color ~* '^#[a-fA-F0-9]{6}$'),
            created timestamptz NOT NULL,
            edited timestamptz NOT NULL,
            pin_count integer NOT NULL
        );"#).execute(&self.pool).await?;

        sqlx::query(r#"CREATE TABLE IF NOT EXISTS board.board_perms (
            board_id uuid NOT NULL REFERENCES board.boards(id),
            perm_id integer,
            user_id text NOT NULL REFERENCES users(id),
            UNIQUE(board_id, user_id)
        );"#).execute(&self.pool).await?;

        sqlx::query(r#"CREATE TABLE IF NOT EXISTS board.pins (
            id uuid primary key unique,
            board_id uuid NOT NULL REFERENCES board.boards(id),
            pin_type integer NOT NULL,
            content text NOT NULL,
            creator_id text NOT NULL REFERENCES users(id),
            created timestamptz NOT NULL,
            edited timestamptz NOT NULL,
            flags integer NOT NULL,
            attachment_paths text[],
            metadata json
        );"#).execute(&self.pool).await?;

        sqlx::query(r#"CREATE TABLE IF NOT EXISTS board.favorites (
            user_id text NOT NULL REFERENCES users(id),
            pin_id uuid NOT NULL REFERENCES board.pins(id),
            UNIQUE(user_id, pin_id)
        );"#).execute(&self.pool).await?;

        sqlx::query(r#"CREATE TABLE IF NOT EXISTS board.pin_history (
            id SERIAL PRIMARY KEY,
            editor text NOT NULL REFERENCES users(id),
            pin_id uuid NOT NULL REFERENCES board.pins(id),
            content text NOT NULL,
            time timestamptz NOT NULL,
            flags integer NOT NULL,
            attachment_paths text[],
            metadata json
        );"#).execute(&self.pool).await?;

        sqlx::query(r#"CREATE TABLE IF NOT EXISTS board.tags (
            id SERIAL PRIMARY KEY,
            name text NOT NULL CHECK(length(name) < 60),
            color text NOT NULL CHECK(color ~* '^#[a-fA-F0-9]{6}$'),
            creator_id text NOT NULL REFERENCES users(id),
            name_lower TEXT GENERATED ALWAYS AS (LOWER (name)) STORED
        );"#).execute(&self.pool).await?;

        sqlx::query(r#"CREATE TABLE IF NOT EXISTS board.tag_ids (
            id integer NOT NULL REFERENCES board.tags(id),
            board_id uuid NOT NULL REFERENCES board.boards(id)
        );"#).execute(&self.pool).await?;

        Ok(())
    }

    pub async fn get_board(&self, board_id: &Uuid) -> Option<board::Board> {
        let perms: HashMap<String, Perm> = sqlx::query("SELECT * FROM board.board_perms WHERE board_id = $1")
            .bind(board_id)
            .map(|row: PgRow| (
                row.get("user_id"),
                Perm { perm_level: row.get("perm_id") }
            ))
            .fetch_all(&self.pool).await.unwrap_or(Vec::new()).into_iter().collect();

        match sqlx::query("SELECT * FROM board.boards WHERE id = $1;")
                .bind(board_id).fetch_one(&self.pool).await {
            Ok(b) => Some(board::Board {
                name: b.get::<String, &str>("name"),
                id: b.get::<Uuid, &str>("id"),
                desc: b.get::<String, &str>("description"),
                creator: b.get::<String, &str>("creator_id"),
                color: b.get::<String, &str>("color"),
                created: b.get::<chrono::DateTime<Utc>, &str>("created"),
                edited: b.get::<chrono::DateTime<Utc>, &str>("edited"),
                perms,
                pin_count: b.get::<i32, &str>("pin_count")
            }),
            Err(_err) => None
        }
    }

    pub async fn create_board(&self, name: String, creator_id: &UserId, desc: String, color: String, perms: HashMap<String, Perm>)
            -> Result<board::Board, sqlx::Error> {
        let mut id: Uuid;
        loop {
            id = Uuid::new_v4();
            if self.get_board(&id).await.is_none() { break; }
        }

        let created = chrono::offset::Utc::now();
        let edited = created.clone();
        let mut tx = self.pool.begin().await?;

        sqlx::query(r#"INSERT INTO board.boards(id, name, description, creator_id, color, created, edited, pin_count)
            VALUES($1, $2, $3, $4, $5, $6, $7, 0);"#)
            .bind(id).bind(name).bind(desc).bind(creator_id).bind(color).bind(created).bind(edited)
            .execute(&mut *tx).await?;

        // Creator gets owner permission by default
        sqlx::query(r#"INSERT INTO board.board_perms(board_id, user_id, perm_id) VALUES($1, $2, $3);"#)
            .bind(id).bind(creator_id.clone()).bind(PermLevel::Owner)
            .execute(&mut *tx).await?;

        for (perm_user_id, val) in perms {
            if perm_user_id == creator_id { continue; }

            let result = sqlx::query(r#"INSERT INTO board.board_perms(board_id, user_id, perm_id) VALUES($1, $2, $3);"#)
                .bind(id).bind(perm_user_id.clone()).bind(val.perm_level.clone())
                .execute(&mut *tx).await;

            // Rollback on error
            if result.is_err() {
                tx.rollback().await?;
                return Err(result.unwrap_err());
            }
        };
        tx.commit().await?;

        return Ok(self.get_board(&id).await.unwrap());
    }

    pub async fn modify_board(&self, user_id: String, board_id: &Uuid, name: Option<String>, desc: Option<String>,
        color: Option<String>, perms: Option<HashMap<String, Perm>>)
            -> Result<board::Board, sqlx::Error> {
        let mut b = self.get_board(&board_id).await.unwrap();
        b.edited = chrono::offset::Utc::now();

        update_if_not_none!(b, name);
        update_if_not_none!(b, desc);
        update_if_not_none!(b, color);

        let mut tx = self.pool.begin().await?;
        sqlx::query("UPDATE board.boards SET name = $2, description = $3, color = $4, edited = $5 WHERE id = $1;")
            .bind(board_id).bind(b.name).bind(b.desc).bind(b.color).bind(b.edited)
            .execute(&mut *tx).await?;

        if perms.is_some() && perms.as_ref().unwrap() != &b.perms {
            let mut perms = perms.unwrap();

            if b.perms.get(&user_id).unwrap().perm_level == PermLevel::Edit {
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
                for (user, perm) in b.perms {
                    if (perm.perm_level == PermLevel::Edit || perm.perm_level == PermLevel::Owner) &&
                            user != user_id {
                        perms.insert(user, perm.clone());
                    }
                }
            }

            // Delete all existing perms, then insert new perms
            sqlx::query(r#"DELETE FROM board.board_perms WHERE board_id = $1;"#)
                .bind(board_id).execute(&mut *tx).await?;

            // Creator gets owner permission by default
            sqlx::query(r#"INSERT INTO board.board_perms(board_id, user_id, perm_id) VALUES($1, $2, $3);"#)
                .bind(board_id).bind(b.creator.clone()).bind(PermLevel::Owner)
                .execute(&mut *tx).await?;

            for (perm_user_id, val) in perms {
                // Ignore board creator: Always owner permission as defined above
                if perm_user_id == b.creator {
                    continue;
                }

                // Ignore users that don't exist
                if sqlx::query(r#"SELECT * FROM users where id = $1;"#)
                        .bind(perm_user_id.clone()).fetch_one(&self.pool).await
                        .is_err() {
                    continue;
                }

                let result = sqlx::query(r#"INSERT INTO board.board_perms(board_id, user_id, perm_id) VALUES($1, $2, $3);"#)
                    .bind(board_id).bind(perm_user_id.clone()).bind(val.perm_level.clone())
                    .execute(&mut *tx).await;
                if result.is_err() {
                    tx.rollback().await?;
                    return Err(result.unwrap_err());
                }
            }
        }

        tx.commit().await?;
        return Ok(self.get_board(&board_id).await.unwrap());
    }

    pub async fn delete_board(&self, board_id: &Uuid) -> Result<(), sqlx::Error> {
        let mut tx = self.pool.begin().await?;
        sqlx::query(r#"DELETE FROM board.favorites USING board.pins t1 WHERE t1.id = pin_id AND t1.board_id = $1;"#)
            .bind(board_id).execute(&mut *tx).await?;
        sqlx::query(r#"DELETE FROM board.pin_history USING board.pins t1 WHERE t1.id = pin_id AND t1.board_id = $1;"#)
            .bind(board_id).execute(&mut *tx).await?;
        sqlx::query(r#"DELETE FROM board.tag_ids WHERE board_id = $1;"#)
            .bind(board_id).execute(&mut *tx).await?;
    
        sqlx::query(r#"DELETE FROM board.pins WHERE board_id = $1;"#)
            .bind(board_id).execute(&mut *tx).await?;
        sqlx::query(r#"DELETE FROM board.board_perms WHERE board_id = $1;"#)
            .bind(board_id).execute(&mut *tx).await?;
        sqlx::query("DELETE FROM board.boards WHERE id = $1;")
            .bind(board_id).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn mass_edit_board_colors(&self, user: &UserId, board_ids: Vec<Uuid>, new_color: &String)
            -> Result<(), sqlx::Error> {
        // Limit board id count to 200
        let end = std::cmp::min(200, board_ids.len());
        let board_ids = &board_ids[0..end];

        // Filter boards to ones the user can edit
        let board_ids = futures::stream::iter(board_ids)
            .filter(|x| async {
                let board = self.get_board(x).await;
                if board.is_none() { return false; }
                let board = board.unwrap();
                return board.perms.contains_key(user) &&
                    (board.perms.get(user).unwrap().perm_level == PermLevel::Owner ||
                    board.perms.get(user).unwrap().perm_level == PermLevel::Edit);
            })
            .collect::<Vec<Uuid>>()
            .await;

        let mut tx = self.pool.begin().await?;
        sqlx::query("UPDATE board.boards SET color = $1 WHERE id = ANY($2);")
            .bind(new_color).bind(board_ids)
            .execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn get_boards(&self, user: &UserId, offset: Option<u32>, limit: Option<u32>,
        not_self: Option<bool>, owner_search: &Option<String>, search_query: &Option<String>,
        sort_by: Option<board::SortBoard>, sort_down: Option<bool>)
            -> Result<Vec<board::Board>, sqlx::Error> {

        let mut owner_search_id = match owner_search {
            None => None,
            Some(owner) => Some(owner).cloned()
        };

        let mut owner_disallow_id: Option<String> = None;
        if not_self.unwrap_or(false) {
            owner_disallow_id = Some(user.to_string());
            owner_search_id = None;
        }

        let sort_condition = sort_by.unwrap_or(board::SortBoard::Created).to_string();
        let mut sort_down_str = "ASC";
        if !sort_down.unwrap_or(true) { sort_down_str = "DESC"; }

        // Only return tables user can access
        Ok(sqlx::query(("SELECT * FROM board.boards
            INNER JOIN board.board_perms ON
                user_id = $4 and board_id = id and
                ($1 is null or name ILIKE '%' || $1 || '%' or description ILIKE '%' || $1 || '%') and
                ($2 is null or creator_id = $2) and
                ($3 is null or creator_id != $3)
            ORDER BY ".to_owned() + &sort_condition + " " + &sort_down_str + " OFFSET $5 LIMIT $6;").as_str())
                .bind(search_query)
                .bind(owner_search_id)
                .bind(owner_disallow_id)
                .bind(user)
                .bind(offset.unwrap_or(0) as i32)
                .bind(cmp::min(100, limit.unwrap_or(20) as i32))
                .map(|row: PgRow| board::Board {
                    name: row.get::<String, &str>("name"),
                    id: row.get::<Uuid, &str>("id"),
                    desc: row.get::<String, &str>("description"),
                    creator: row.get::<String, &str>("creator_id"),
                    color: row.get::<String, &str>("color"),
                    created: row.get::<chrono::DateTime<Utc>, &str>("created"),
                    edited: row.get::<chrono::DateTime<Utc>, &str>("edited"),
                    perms: HashMap::from([(
                        user.to_string(), Perm { perm_level: row.get::<PermLevel, &str>("perm_id") }
                    )]),
                    pin_count: row.get::<i32, &str>("pin_count"),
                })
                .fetch_all(&self.pool).await?)
    }

    pub async fn get_mass_board_share_perms(&self, user: &UserId, ids: &Vec<Uuid>)
            -> Result<HashMap<String, board::MassBoardShareUser>, sqlx::Error> {
        // Limit id count to 200
        let end = std::cmp::min(200, ids.len());
        let ids = &ids[0..end];

        // user: (name, perm)
        let mut result: HashMap<String,  board::MassBoardShareUser> = HashMap::new();

        // First: get perms for all allowed tables (user can edit or is owner)
        let allowed_board_ids = sqlx::query("SELECT DISTINCT board_id FROM board.boards
            INNER JOIN board.board_perms ON
            user_id = $1 and board_id = ANY($2) and (perm_id = 3 or perm_id = 4)
            LIMIT 200;")
                .bind(user).bind(ids)
                .map(|row: PgRow| row.get::<Uuid, &str>("board_id"))
                .fetch_all(&self.pool).await?;
        let board_len = allowed_board_ids.len();

        // Get all users with these allowed board ids, ensuring they exist in all boards
        let users = sqlx::query("SELECT perm_id, id, board_id, name FROM users, board.board_perms WHERE
            user_id = id and board_id = ANY($2) LIMIT 200;")
                .bind(user).bind(allowed_board_ids)
                .map(|row: PgRow| (
                    row.get::<String, &str>("id"),
                    row.get::<String, &str>("name"),
                    Perm { perm_level: row.get("perm_id") }
                ))
                .fetch_all(&self.pool).await?;

        // Remove users that do not have the same perm level in all boards
        let mut bad_users = HashSet::new();
        let mut user_count: HashMap<String, u32> = HashMap::new();
        for u in users {
            if result.contains_key(&u.0) {
                if u.2.perm_level != result.get(&u.0).unwrap().perm.perm_level {
                    result.remove(&u.0);
                    user_count.remove(&u.0);
                    bad_users.insert(u.0);
                } else {
                    *user_count.get_mut(&u.0).unwrap() += 1;
                }
            } else if !bad_users.contains(&u.0) {
                user_count.insert(u.0.clone(), 1);
                result.insert(u.0, board::MassBoardShareUser {
                    name: u.1,
                    perm: u.2
                });
            }
        }
        for (key, value) in user_count {
            if value as usize != board_len {
                result.remove(&key);
            }
        }

        Ok(result)
    }

    // Returns number of boards changed
    pub async fn mass_change_board_share_perms(&self, user: &UserId, ids: &Vec<Uuid>,
                perms_to_add: &HashMap<String, Perm>, users_to_delete: &Vec<String>)
            -> Result<i32, sqlx::Error> {
        // Limit id count to 200
        let end = std::cmp::min(200, ids.len());
        let ids = &ids[0..end];

        // First: get perms for all allowed tables (user can edit or is owner)
        let mut tx = self.pool.begin().await?;
        let allowed_board_ids_owner = sqlx::query("SELECT DISTINCT board_id FROM board.boards
            INNER JOIN board.board_perms ON user_id = $1 and board_id = ANY($2) and perm_id = $3 LIMIT 200;")
                .bind(user).bind(ids).bind(PermLevel::Owner)
                .map(|row: PgRow| row.get::<Uuid, &str>("board_id"))
                .fetch_all(&mut *tx).await?;

        let allowed_board_ids_edit = sqlx::query("SELECT DISTINCT board_id FROM board.boards
            INNER JOIN board.board_perms ON user_id = $1 and board_id = ANY($2) and perm_id = $3 LIMIT 200;")
                .bind(user).bind(ids).bind(PermLevel::Edit)
                .map(|row: PgRow| row.get::<Uuid, &str>("board_id"))
                .fetch_all(&mut *tx).await?;

        // For boards the user has an edit perm on, they cannot change the user of anyone
        // except with perm below edit (3)
        let can_edit_users = sqlx::query("SELECT DISTINCT board_id, user_id FROM board.boards
            INNER JOIN board.board_perms ON board_id = ANY($1) and perm_id < $2 LIMIT 200;")
                .bind(allowed_board_ids_edit.clone()).bind(PermLevel::Edit)
                .map(|row: PgRow| row.get::<String, &str>("user_id"))
                .fetch_all(&mut *tx).await?;

        for (username, perm) in perms_to_add {
            // Ignore users that don't exist
            if sqlx::query(r#"SELECT * FROM users where id = $1;"#)
                    .bind(username.clone()).fetch_one(&self.pool).await
                    .is_err() {
                continue;
            }

            // Owner perm boards: free to update any value
            if allowed_board_ids_owner.len() > 0 {
                sqlx::query(r#"INSERT INTO board.board_perms(board_id, user_id, perm_id)
                        VALUES(unnest($1), $2, $3) ON CONFLICT (board_id, user_id) DO UPDATE SET
                        perm_id = $3;"#)
                    .bind(allowed_board_ids_owner.clone())
                    .bind(username.clone())
                    .bind(perm.perm_level.clone())
                    .execute(&mut *tx).await?;
            }
        
            // Edit board: cannot insert owner perm
            if can_edit_users.len() > 0 && allowed_board_ids_edit.len() > 0 {
                let mut new_perm = perm.clone();
                if new_perm.perm_level == PermLevel::Owner { new_perm.perm_level = PermLevel::Edit; }

                sqlx::query(r#"INSERT INTO board.board_perms(board_id, user_id, perm_id)
                        VALUES(unnest($1), $2, $3) ON CONFLICT (board_id, user_id) DO UPDATE SET
                        perm_id = $3;"#)
                    .bind(allowed_board_ids_edit.clone()).bind(can_edit_users.clone())
                    .bind(new_perm.perm_level.clone())
                    .execute(&mut *tx).await?;
            }
        }

        // Delete permissions
        if allowed_board_ids_owner.len() > 0 && users_to_delete.len() > 0 {
            sqlx::query(r#"DELETE FROM board.board_perms USING board.boards t1 WHERE user_id != t1.creator_id AND t1.id = board_id AND board_id = ANY($1) AND user_id = ANY($2);"#)
                .bind(allowed_board_ids_owner.clone()).bind(users_to_delete.clone())
                .execute(&mut *tx).await?;
        }
        if allowed_board_ids_edit.len() > 0 && users_to_delete.len() > 0 && can_edit_users.len() > 0 {
            sqlx::query(r#"DELETE FROM board.board_perms USING board.boards t1 WHERE user_id != t1.creator_id AND t1.id = board_id AND board_id = ANY($1) AND user_id = ANY($2) AND user_id = ANY($3);"#)
                .bind(allowed_board_ids_edit.clone()).bind(users_to_delete.clone()).bind(can_edit_users.clone())
                .execute(&mut *tx).await?;
        }

        // Ensure board creator always has owner perm
        if allowed_board_ids_owner.len() > 0 {
            sqlx::query(r#"UPDATE board.board_perms SET perm_id = $2 FROM board.boards
                WHERE creator_id = user_id AND id = board_id AND board_id = ANY($1);"#)
                .bind(allowed_board_ids_owner.clone()).bind(PermLevel::Owner)
                .execute(&mut *tx).await?;
        }
        if allowed_board_ids_edit.len() > 0 {
            sqlx::query(r#"UPDATE board.board_perms SET perm_id = $2 FROM board.boards
                WHERE creator_id = user_id AND id = board_id AND board_id = ANY($1);"#)
                .bind(allowed_board_ids_edit.clone()).bind(PermLevel::Owner)
                .execute(&mut *tx).await?;
        }
        tx.commit().await?;

        Ok((allowed_board_ids_edit.len() + allowed_board_ids_owner.len()) as i32)
    }


    // --------------- Pins ----------------------
    pub async fn get_pin(&self, pin_id: &Uuid) -> Option<pin::Pin> {
        match sqlx::query("SELECT * FROM board.pins WHERE id = $1;")
                .bind(pin_id).fetch_one(&self.pool).await {
            Ok(p) => Some(pin::Pin {
                pin_id: *pin_id,
                board_id: p.get::<Uuid, &str>("board_id"),
                pin_type: num::FromPrimitive::from_u32(p.get::<i32, &str>("pin_type") as u32).unwrap(),
                content: p.get::<String, &str>("content"),
                creator: p.get::<String, &str>("creator_id"),
                created: p.get::<chrono::DateTime<Utc>, &str>("created"),
                edited: p.get::<chrono::DateTime<Utc>, &str>("edited"),
                flags: pin::PinFlags::from_bits_truncate(p.get::<i32, &str>("flags") as u64),
                attachment_paths: p.get::<Option<Vec<String>>, &str>("attachment_paths").unwrap_or(Vec::new()),
                metadata: p.get::<Option<Value>, &str>("metadata").unwrap_or(serde_json::from_str("{}").ok()?)
            }),
            Err(_err) => None
        }
    }

    pub async fn create_pin(&self, creator: &UserId, pin_type: pin::PinType, board_id: &Uuid, content: String,
            attachment_paths: Vec<String>, flags: pin::PinFlags, metadata: Value)
            -> Result<pin::Pin, sqlx::Error> {
        let mut id: Uuid;
        loop {
            id = Uuid::new_v4();
            if self.get_pin(&id).await.is_none() { break; }
        }
        
        let content = clean_html(&content);
        let created = chrono::offset::Utc::now();
        let edited = created.clone();
        let mut tx = self.pool.begin().await?;

        sqlx::query(r#"INSERT INTO board.pins(id, board_id, pin_type, content, creator_id, created, edited, flags, attachment_paths, metadata)
            VALUES($1, $2, $3, $4, $5, $6, $7, $8, $9, $10);"#)
            .bind(id).bind(board_id).bind(pin_type as i16).bind(content).bind(creator)
            .bind(created).bind(edited).bind(flags.bits() as i32).bind(attachment_paths).bind(metadata)
            .execute(&mut *tx).await?;

        // Update pin count
        sqlx::query(r#"UPDATE board.boards SET pin_count = pin_count + 1 WHERE id = $1;"#)
            .bind(board_id).execute(&mut *tx).await?;
        tx.commit().await?;

        return Ok(self.get_pin(&id).await.unwrap());
    }

    pub async fn modify_pin(&self, user_id: &UserId, pin_id: &Uuid, pin_type: Option<pin::PinType>, board_id: &Option<Uuid>,
            content: Option<String>, attachment_paths: Option<Vec<String>>, flags: Option<pin::PinFlags>, metadata: Option<Value>)
            -> Result<pin::Pin, sqlx::Error> {
        let mut p = self.get_pin(&pin_id).await.unwrap();
        p.edited = chrono::offset::Utc::now();

        let original_content = p.content.clone();
        let original_attachment_paths = p.attachment_paths.clone();
        let original_flags = p.flags.clone();
        let original_metadata = p.metadata.clone();

        update_if_not_none!(p, pin_type);
        update_if_not_none!(p, board_id);
        update_if_not_none!(p, content);
        update_if_not_none!(p, attachment_paths);
        update_if_not_none!(p, flags);
        update_if_not_none!(p, metadata);

        p.content = clean_html(&p.content);

        let mut tx = self.pool.begin().await?;
        sqlx::query("UPDATE board.pins SET pin_type = $2, content = $3, edited = $4, flags = $5, attachment_paths = $6, metadata = $7 WHERE id = $1;")
            .bind(pin_id).bind(p.pin_type as i16).bind(p.content.clone()).bind(p.edited)
            .bind(p.flags.bits() as i32).bind(p.attachment_paths.clone()).bind(p.metadata.clone())
            .execute(&mut *tx).await?;
        tx.commit().await?;

        if original_content != p.content || original_flags.bits() != p.flags.bits() || original_metadata != p.metadata ||
                original_attachment_paths != p.attachment_paths {
            self.add_to_pin_history(&p.pin_id, user_id,
                &original_content, &original_attachment_paths, &original_flags, &original_metadata).await?;
        }

        return Ok(self.get_pin(&pin_id).await.unwrap());
    }

    pub async fn delete_pin(&self, pin_id: &Uuid) -> Result<(), sqlx::Error> {
        // Delete favorites + history that links to deleted pins
        let mut tx = self.pool.begin().await?;
        sqlx::query("DELETE FROM board.favorites WHERE pin_id = $1;")
            .bind(pin_id).execute(&mut *tx).await?;
        sqlx::query("DELETE FROM board.pin_history WHERE pin_id = $1;")
            .bind(pin_id).execute(&mut *tx).await?;
    
        let board_id = sqlx::query("DELETE FROM board.pins WHERE id = $1 returning board_id;")
            .bind(pin_id).fetch_one(&mut *tx).await?;
        let board_id = board_id.get::<Uuid, &str>("board_id");
        sqlx::query(r#"UPDATE board.boards SET pin_count = pin_count - 1 WHERE id = $1;"#)
            .bind(board_id).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn mass_edit_pin_flags(&self, user: &UserId, pin_ids: Vec<Uuid>, flags: pin::PinFlags, add_flags: bool)
            -> Result<(), sqlx::Error> {
        // Limit pin id count to 100
        let end = std::cmp::min(100, pin_ids.len());
        let pin_ids = &pin_ids[0..end];

        // Filter pins to ones the user can edit
        let pin_ids = futures::stream::iter(pin_ids)
            .filter(|x| async { self.can_edit_pin(user, x).await })
            .collect::<Vec<Uuid>>()
            .await;

        let edited = chrono::offset::Utc::now();

        let mut tx = self.pool.begin().await?;
        if add_flags {
            sqlx::query("UPDATE board.pins SET edited = $1, flags = flags | $2 WHERE id = ANY($3);")
                .bind(edited).bind(flags.bits() as i32).bind(pin_ids)
                .execute(&mut *tx).await?;
        } else {
            sqlx::query("UPDATE board.pins SET edited = $1, flags = flags & ~($2) WHERE id = ANY($3);")
                .bind(edited).bind(flags.bits() as i32).bind(pin_ids)
                .execute(&mut *tx).await?;
        }
        tx.commit().await?;
        Ok(())
    }

    pub async fn mass_edit_pin_colors(&self, user: &UserId, pin_ids: Vec<Uuid>, new_color: &String)
            -> Result<(), sqlx::Error> {
        // Limit pin id count to 100
        let end = std::cmp::min(100, pin_ids.len());
        let pin_ids = &pin_ids[0..end];

        // Filter pins to ones the user can edit
        let pin_ids = futures::stream::iter(pin_ids)
            .filter(|x| async { self.can_edit_pin(user, x).await })
            .collect::<Vec<Uuid>>()
            .await;

        let edited = chrono::offset::Utc::now();

        // Ensure new_color soemwhat resembles a hex string
        if new_color.len() > 7 || !new_color.chars().all(|x| x == '#' || x.is_alphabetic() || x.is_numeric()) {
            return Ok(());
        }

        let mut tx = self.pool.begin().await?;
        sqlx::query(("UPDATE board.pins SET edited = $1, metadata = jsonb_set(metadata::jsonb, '{color}', '\"".to_owned() + new_color + "\"')::json WHERE id = ANY($2);").as_str())
            .bind(edited).bind(pin_ids)
            .execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn mass_delete_pins(&self, user: &UserId, pin_ids: Vec<Uuid>)
            -> Result<(), sqlx::Error> {
        // Limit pin id count to 100
        let end = std::cmp::min(100, pin_ids.len());
        let pin_ids = &pin_ids[0..end];

        // Filter pins to ones the user can edit
        let pin_ids = futures::stream::iter(pin_ids)
            .filter(|x| async { self.can_edit_pin(user, x).await })
            .collect::<Vec<Uuid>>()
            .await;

        let mut tx = self.pool.begin().await?;
        for pin_id in pin_ids {
            sqlx::query("DELETE FROM board.pin_history WHERE pin_id = $1;")
                .bind(pin_id).execute(&mut *tx).await?;
            sqlx::query("DELETE FROM board.favorites WHERE pin_id = $1;")
                .bind(pin_id).execute(&mut *tx).await?;

            let board_id = sqlx::query("DELETE FROM board.pins WHERE id = $1 returning board_id;")
                .bind(pin_id).fetch_one(&mut *tx).await?;
            let board_id = board_id.get::<Uuid, &str>("board_id");
            sqlx::query(r#"UPDATE board.boards SET pin_count = pin_count - 1 WHERE id = $1;"#)
                .bind(board_id).execute(&mut *tx).await?;
        }
        tx.commit().await?;
        Ok(())
    }

    pub async fn get_pins(&self, user: &UserId, board_id: &Option<Uuid>, offset: Option<u32>, limit: Option<u32>,
            creator: &Option<String>, search_query: &Option<String>, sort_by: Option<pin::SortPin>, sort_down: Option<bool>)
                -> Result<Vec<pin::Pin>, sqlx::Error> {
        let sort_condition = sort_by.unwrap_or(pin::SortPin::Created).to_string();
        let mut sort_down_str = "DESC";
        if !sort_down.unwrap_or(true) { sort_down_str = "ASC"; }

        // Only return pins the user can view
        Ok(sqlx::query(("SELECT p2.*, p1.user_id, p1.board_id FROM board.pins p2
            INNER JOIN board.board_perms p1 ON
                p1.user_id = $4 and p1.board_id = p2.board_id and
                ($1 is null or p1.board_id = $1) and
                ($2 is null or p2.content ILIKE '%' || $2 || '%') and
                ($3 is null or p2.creator_id = $3)
            ORDER BY CASE when (flags & 4 = 4) then 2 when (flags & 2 = 2) then 0 else 1 end ".to_owned() +
            sort_down_str + "," + &sort_condition + " " + sort_down_str + " OFFSET $5 LIMIT $6;").as_str())
                .bind(board_id)
                .bind(search_query)
                .bind(creator)
                .bind(user)
                .bind(offset.unwrap_or(0) as i32)
                .bind(cmp::min(100, limit.unwrap_or(20) as i32))
                .map(|row: PgRow| pin::Pin {
                    board_id: row.get::<Uuid, &str>("board_id"),
                    pin_id: row.get::<Uuid, &str>("id"),
                    pin_type: num::FromPrimitive::from_u32(row.get::<i32, &str>("pin_type") as u32).unwrap(),
                    content: row.get::<String, &str>("content"),
                    creator: row.get::<String, &str>("creator_id"),
                    created: row.get::<chrono::DateTime<Utc>, &str>("created"),
                    edited: row.get::<chrono::DateTime<Utc>, &str>("edited"),
                    flags: pin::PinFlags::from_bits_truncate(row.get::<i32, &str>("flags") as u64),
                    attachment_paths: row.get::<Option<Vec<String>>, &str>("attachment_paths").unwrap_or(Vec::new()),
                    metadata: row.get::<Option<Value>, &str>("metadata").unwrap_or(serde_json::from_str("{}").unwrap())
                })
                .fetch_all(&self.pool).await?)
    }

    pub async fn get_perms_for_board(&self, user: &UserId, board_id: &Uuid) -> Option<Perm> {
        let board = self.get_board(board_id).await;
        if board.is_none() { return None; }
        let board = board.unwrap();

        let perm = board.perms.get(user);
        if perm.is_none() { return None; }
        return Some(perm.unwrap().clone());
    }

    pub async fn get_perms_for_pin(&self, user: &UserId, pin_id: &Uuid) -> Option<Perm> {
        let pin = self.get_pin(pin_id).await;
        if pin.is_none() { return None; }
        return self.get_perms_for_board(user, &pin.unwrap().board_id).await;
    }

    pub async fn can_edit_pin(&self, user: &UserId, pin_id: &Uuid) -> bool {
        let perm = self.get_perms_for_pin(user, pin_id).await;
        if perm.is_none() { return false; }

        let perm = perm.unwrap().perm_level;

        // "Edit" or "Owner" are always free to edit
        if perm == PermLevel::Edit || perm == PermLevel::Owner {
            return true;
        }
        // SelfEdit can edit only if pin creator is self
        if perm == PermLevel::SelfEdit {
            let pin = self.get_pin(pin_id).await;
            return pin.is_some() && pin.unwrap().creator == user;
        }
        return false;
    }

    pub async fn add_favorites(&self, user: &UserId, pin_ids: &Vec<Uuid>)
            -> Result<(), sqlx::Error> {
        // Limit pin id count to 100
        let end = std::cmp::min(100, pin_ids.len());
        let pin_ids = &pin_ids[0..end];

        sqlx::query(r#"INSERT INTO board.favorites(user_id, pin_id) VALUES($1, unnest($2)) ON CONFLICT DO NOTHING;"#)
            .bind(user).bind(pin_ids)
            .execute(&self.pool).await?;
        return Ok(());
    }

    pub async fn remove_favorites(&self, user: &UserId, pin_ids: &Vec<Uuid>)
            -> Result<(), sqlx::Error> {
        // Limit pin id count to 100
        let end = std::cmp::min(100, pin_ids.len());
        let pin_ids = &pin_ids[0..end];

        sqlx::query(r#"DELETE FROM board.favorites WHERE user_id = $1 and pin_id = ANY($2);"#)
            .bind(user).bind(pin_ids)
            .execute(&self.pool).await?;
        return Ok(());
    }

    pub async fn get_favorites(&self, user: &UserId,
            offset: Option<u32>, limit: Option<u32>,
            sort_by: Option<pin::SortPin>, sort_down: Option<bool>)
            -> Result<Vec<pin::Pin>, sqlx::Error> {
        let sort_condition = sort_by.unwrap_or(pin::SortPin::Created).to_string();
        let mut sort_down_str = "DESC";
        if !sort_down.unwrap_or(true) { sort_down_str = "ASC"; }
            
        Ok(sqlx::query(("SELECT p2.* FROM board.pins p2
                INNER JOIN board.favorites ON user_id = $1 and id = pin_id
                INNER JOIN board.board_perms p1 ON
                    p1.user_id = $1 and p1.board_id = p2.board_id
                ORDER BY CASE when (flags & 4 = 4) then 2 when (flags & 2 = 2) then 0 else 1 end ".to_owned() +
                sort_down_str + "," + &sort_condition + " " + sort_down_str + "
                OFFSET $2 LIMIT $3;").as_str())
            .bind(user)
            .bind(offset.unwrap_or(0) as i32)
            .bind(cmp::min(100, limit.unwrap_or(20) as i32))
            .map(|row: PgRow| pin::Pin {
                board_id: row.get::<Uuid, &str>("board_id"),
                pin_id: row.get::<Uuid, &str>("id"),
                pin_type: num::FromPrimitive::from_u32(row.get::<i32, &str>("pin_type") as u32).unwrap(),
                content: row.get::<String, &str>("content"),
                creator: row.get::<String, &str>("creator_id"),
                created: row.get::<chrono::DateTime<Utc>, &str>("created"),
                edited: row.get::<chrono::DateTime<Utc>, &str>("edited"),
                flags: pin::PinFlags::from_bits_truncate(row.get::<i32, &str>("flags") as u64),
                attachment_paths: row.get::<Option<Vec<String>>, &str>("attachment_paths").unwrap_or(Vec::new()),
                metadata: row.get::<Option<Value>, &str>("metadata").unwrap_or(serde_json::from_str("{}").unwrap())
            })
            .fetch_all(&self.pool).await?)
    }

    pub async fn check_favorites(&self, user: &UserId, pin_ids: &Vec<Uuid>)
            -> Result<Vec<Uuid>, sqlx::Error> {
        // Limit pin id count to 100
        let end = std::cmp::min(100, pin_ids.len());
        let pin_ids = &pin_ids[0..end];

        Ok(sqlx::query("SELECT pin_id FROM board.favorites WHERE user_id = $1 and pin_id = ANY($2);")
            .bind(user).bind(pin_ids)
            .map(|row: PgRow| row.get::<Uuid, &str>("pin_id"))
            .fetch_all(&self.pool).await?)
    }

    pub async fn get_pin_history_preview(&self, pin_id: &Uuid, user: &UserId)
             -> Result<Vec<pin::PinHistoryAbridged>, sqlx::Error> {
        // Check if user has permission to view pin
        let perm = self.get_perms_for_pin(user, pin_id).await;
        if perm.is_none() { return Ok(Vec::new()); }

        Ok(sqlx::query("SELECT * FROM board.pin_history WHERE pin_id = $1  ORDER BY time DESC LIMIT 200;")
                .bind(pin_id)
                .map(|row: PgRow| pin::PinHistoryAbridged {
                    id: row.get::<i32, &str>("id"),
                    editor: row.get::<String, &str>("editor"),
                    time: row.get::<chrono::DateTime<Utc>, &str>("time")
                })
                .fetch_all(&self.pool).await?)
    }

    pub async fn get_pin_history(&self, pin_id: &Uuid, history_id: i32, user: &UserId)
             -> Result<Option<pin::PinHistory>, sqlx::Error> {
        // Check if user has permission to view pin
        let perm = self.get_perms_for_pin(user, pin_id).await;
        if perm.is_none() { return Ok(None); }

        Ok(Some(sqlx::query("SELECT * FROM board.pin_history WHERE pin_id = $1 AND id = $2 ORDER BY time DESC LIMIT 1;")
                .bind(pin_id).bind(history_id)
                .map(|row: PgRow| pin::PinHistory {
                    editor: row.get::<String, &str>("editor"),
                    time: row.get::<chrono::DateTime<Utc>, &str>("time"),
                    content: row.get::<String, &str>("content"),
                    flags: pin::PinFlags::from_bits_truncate(row.get::<i32, &str>("flags") as u64),
                    attachment_paths: row.get::<Option<Vec<String>>, &str>("attachment_paths").unwrap_or(Vec::new()),
                    metadata: row.get::<Option<Value>, &str>("metadata").unwrap_or(serde_json::from_str("{}").unwrap())
                })
                .fetch_one(&self.pool).await?))
    }

    async fn add_to_pin_history(&self, pin_id: &Uuid, user: &UserId, content: &String,
            attachments: &Vec<String>, flags: &PinFlags, metadata: &Value)
            -> Result<(), sqlx::Error> {
        // If last update was too recent and by the same user update previous entry
        // then overwrite it instead of creating a new one
        let mut update_instead = false;
        let now = chrono::offset::Utc::now();

        match sqlx::query("SELECT * FROM board.pin_history WHERE pin_id = $1 AND id IN(SELECT MAX(id) from board.pin_history);")
                .bind(pin_id).fetch_one(&self.pool).await {
            Ok(history) => {
                update_instead = history.get::<String, &str>("editor") == user &&
                    history.get::<chrono::DateTime<Utc>, &str>("time") > now - chrono::Duration::minutes(5);
            },
            Err(_err) => {}
        }
        if update_instead {
            sqlx::query(r#"UPDATE board.pin_history
                SET editor=$1, pin_id=$2, content=$3, time=$4, flags=$5, attachment_paths=$6, metadata=$7
                WHERE id IN(SELECT MAX(id) from board.pin_history);"#)
                .bind(user).bind(pin_id).bind(content).bind(now)
                .bind(flags.bits() as i32).bind(attachments).bind(metadata)
                .execute(&self.pool).await?;
            return Ok(());
        }

        // Delete old history for this pin
        sqlx::query("DELETE FROM board.pin_history WHERE pin_id = $1 AND
                    id != all(array(SELECT id FROM board.pin_history WHERE pin_id = $1 ORDER BY time DESC LIMIT 100));")
                .bind(pin_id).execute(&self.pool).await?;

        sqlx::query(r#"INSERT INTO board.pin_history(editor, pin_id, content, time, flags, attachment_paths, metadata)
            VALUES($1, $2, $3, $4, $5, $6, $7);"#)
            .bind(user).bind(pin_id).bind(content).bind(now)
            .bind(flags.bits() as i32).bind(attachments).bind(metadata)
            .execute(&self.pool).await?;
        Ok(())
    }

    pub async fn get_tag(&self, creator_id: &UserId, id: i32)
            -> Result<board::Tag, sqlx::Error> {
        let board_ids = sqlx::query("SELECT * FROM board.tag_ids WHERE id = $1;")
            .bind(id).map(|b: PgRow| b.get::<Uuid, &str>("board_id"))
            .fetch_all(&self.pool).await?;

        Ok(sqlx::query("SELECT * FROM board.tags WHERE id = $1 AND creator_id = $2;")
            .bind(id).bind(creator_id)
            .map(|b: PgRow| board::Tag {
                creator_id: b.get::<String, &str>("creator_id"),
                name: b.get::<String, &str>("name"),
                id: b.get::<i32, &str>("id"),
                color: b.get::<String, &str>("color"),
                board_ids: board_ids.clone()
            })
            .fetch_one(&self.pool).await?)
    }

    pub async fn get_tags(&self, creator_id: &UserId)
            -> Result<Vec<board::Tag>, sqlx::Error> {
        let mut tags = sqlx::query("SELECT * FROM board.tags WHERE creator_id = $1 ORDER BY name_lower ASC LIMIT 200;")
                .bind(creator_id)
                .map(|b: PgRow| board::Tag {
                    creator_id: b.get::<String, &str>("creator_id"),
                    name: b.get::<String, &str>("name"),
                    id: b.get::<i32, &str>("id"),
                    color: b.get::<String, &str>("color"),
                    board_ids: Vec::new()
                })
                .fetch_all(&self.pool).await?;

        let tag_ids: Vec<i32> = tags.clone().into_iter().map(|x| x.id).collect();
        for (i, tag_id) in tag_ids.iter().enumerate() {
            tags[i].board_ids = sqlx::query("SELECT * FROM board.tag_ids WHERE id = $1;")
                .bind(tag_id).map(|b: PgRow| b.get::<Uuid, &str>("board_id"))
                .fetch_all(&self.pool).await?
        }

        Ok(tags)
    }

    pub async fn create_tag(&self, creator_id: &UserId, name: &str, color: &str, board_ids: &Vec<Uuid>)
            -> Result<(), sqlx::Error> {
        let id = sqlx::query(r#"INSERT INTO board.tags(name, color, creator_id) VALUES($1, $2, $3) RETURNING id;"#)
            .bind(name).bind(color).bind(creator_id)
            .fetch_one(&self.pool).await?;

        sqlx::query(r#"INSERT INTO board.tag_ids(id, board_id) VALUES($1, unnest($2));"#)
            .bind(id.get::<i32, &str>("id")).bind(board_ids)
            .execute(&self.pool).await?;

        return Ok(());
    }

    pub async fn modify_tag(&self, creator_id: &UserId, id: i32, name: Option<String>, color: Option<String>, board_ids: Option<Vec<Uuid>>)
            -> Result<(), sqlx::Error> {
        let tag = self.get_tag(creator_id, id).await?;

        // Check if user is allowed to modify this id
        if tag.creator_id != creator_id { return Ok(()); }
        
        let board_ids = board_ids.unwrap_or(tag.board_ids);
        let name = name.unwrap_or(tag.name);
        let color = color.unwrap_or(tag.color);

        sqlx::query(r#"UPDATE board.tags SET name = $1, color = $2 WHERE id = $3"#)
            .bind(name).bind(color).bind(id)
            .execute(&self.pool).await?;
        sqlx::query(r#"DELETE FROM board.tag_ids WHERE id = $1;"#)
            .bind(id).execute(&self.pool).await?;
        sqlx::query(r#"INSERT INTO board.tag_ids(id, board_id) VALUES($1, unnest($2));"#)
            .bind(id).bind(board_ids)
            .execute(&self.pool).await?;

        return Ok(());
    }

    pub async fn tag_add_remove_boards(&self, creator_id: &UserId, id: i32, board_ids_to_add: &Vec<Uuid>, board_ids_to_delete: &Vec<Uuid>)
            -> Result<(), sqlx::Error> {
        let tag = self.get_tag(creator_id, id).await?;

        // Check if user is allowed to modify this id
        if tag.creator_id != creator_id { return Ok(()); }

        if board_ids_to_delete.len() > 0 { 
            sqlx::query(r#"DELETE FROM board.tag_ids WHERE id = $1 AND board_id = ANY($1);"#)
                .bind(id).bind(board_ids_to_delete).execute(&self.pool).await?;
        }
        if board_ids_to_add.len() > 0 {
            sqlx::query(r#"INSERT INTO board.tag_ids(id, board_id) VALUES($1, unnest($2));"#)
                .bind(id).bind(board_ids_to_add).execute(&self.pool).await?;
        }

        return Ok(());
    }

    pub async fn delete_tags(&self, creator_id: &UserId, ids: &Vec<i32>)
            -> Result<(), sqlx::Error> {
        // Filter ids by ones the user created
        let allowed_tag_ids = sqlx::query("SELECT DISTINCT id FROM board.tags WHERE creator_id = $1 AND id = ANY($2) LIMIT 200;")
                .bind(creator_id).bind(ids)
                .map(|row: PgRow| row.get::<i32, &str>("id"))
                .fetch_all(&self.pool).await?;

        sqlx::query(r#"DELETE FROM board.tag_ids WHERE id = ANY($1);"#)
            .bind(allowed_tag_ids.clone()).execute(&self.pool).await?;
        sqlx::query(r#"DELETE FROM board.tags WHERE id = ANY($1);"#)
            .bind(allowed_tag_ids).execute(&self.pool).await?;
        return Ok(());
    }
}
