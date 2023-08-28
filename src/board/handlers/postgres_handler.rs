use crate::shared::types::account::{UserId, Perm, PermLevel};
use crate::board::types::pin;
use crate::board::types::board;
use crate::shared::util::config;

use chrono;
use chrono::Utc;
use uuid::Uuid;
use std::collections::HashMap;
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

    pub async fn create_board(&self, name: String, creator_id: &str, desc: String, color: String, perms: HashMap<String, Perm>)
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
        sqlx::query(r#"DELETE FROM board.pins WHERE board_id = $1;"#)
            .bind(board_id).execute(&self.pool).await?;
        sqlx::query(r#"DELETE FROM board.board_perms WHERE board_id = $1;"#)
            .bind(board_id).execute(&self.pool).await?;
        sqlx::query("DELETE FROM board.boards WHERE id = $1;")
            .bind(board_id).execute(&self.pool).await?;
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

    pub async fn modify_pin(&self, pin_id: &Uuid, pin_type: Option<pin::PinType>, board_id: &Option<Uuid>,
            content: Option<String>, attachment_paths: Option<Vec<String>>, flags: Option<pin::PinFlags>, metadata: Option<Value>)
            -> Result<pin::Pin, sqlx::Error> {
        let mut p = self.get_pin(&pin_id).await.unwrap();
        p.edited = chrono::offset::Utc::now();

        update_if_not_none!(p, pin_type);
        update_if_not_none!(p, board_id);
        update_if_not_none!(p, content);
        update_if_not_none!(p, attachment_paths);
        update_if_not_none!(p, flags);
        update_if_not_none!(p, metadata);

        let mut tx = self.pool.begin().await?;
        sqlx::query("UPDATE board.pins SET pin_type = $2, content = $3, edited = $4, flags = $5, attachment_paths = $6, metadata = $7 WHERE id = $1;")
            .bind(pin_id).bind(p.pin_type as i16).bind(p.content).bind(p.edited)
            .bind(p.flags.bits() as i32).bind(p.attachment_paths).bind(p.metadata)
            .execute(&mut *tx).await?;

        tx.commit().await?;
        return Ok(self.get_pin(&pin_id).await.unwrap());
    }

    pub async fn delete_pin(&self, pin_id: &Uuid) -> Result<(), sqlx::Error> {
        let board_id = sqlx::query("DELETE FROM board.pins WHERE id = $1 returning board_id;")
            .bind(pin_id).fetch_one(&self.pool).await?;
        let board_id = board_id.get::<Uuid, &str>("board_id");
        sqlx::query(r#"UPDATE board.boards SET pin_count = pin_count - 1 WHERE id = $1;"#)
            .bind(board_id).execute(&self.pool).await?;
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
        if new_color.len() > 7 || !new_color.chars().all(|x| x == '#' || x.is_alphabetic()) {
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
}
