use crate::shared::types::account::{Account, UserId, Perm};
use crate::board::types::pin;
use crate::board::types::board;
use crate::shared::util::config;

use chrono;
use chrono::Utc;
use uuid::Uuid;
use serde_json::Value;
use json_value_merge::Merge;

use sqlx::Row;
use sqlx::postgres::{PgPool, PgRow};

pub struct PostgresHandler {
    pool: PgPool
}

impl PostgresHandler {
    pub async fn new() -> Result<PostgresHandler, sqlx::Error> {
        Ok(PostgresHandler { pool: config::get_pool().await })
    }

    // Called on first launch for setup
    pub async fn init(&mut self) -> Result<(), sqlx::Error> {
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
            edited timestamptz NOT NULL
        );"#).execute(&self.pool).await?;

        sqlx::query(r#"CREATE TABLE IF NOT EXISTS board.board_perms (
            table_id uuid NOT NULL REFERENCES board.boards(id),
            perm_id integer,
            user_id text NOT NULL REFERENCES users(id),
            UNIQUE(table_id, user_id)
        );"#).execute(&self.pool).await?;
        Ok(())
    }

    pub async fn get_board(&self, board_id: &Uuid) -> Option<board::Board> {
        let perms: Vec<Perm> = sqlx::query("SELECT * FROM board.board_perms WHERE table_id = $1")
            .bind(board_id)
            .map(|row: PgRow| Perm {
                user_id: row.get("user_id"),
                perm_level: row.get("perm_id"),
            })
            .fetch_all(&self.pool).await.unwrap_or(Vec::new());

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
                perms
            }),
            Err(_err) => None
        }
    }

    pub async fn create_board(&mut self, name: String, creator_id: &str, desc: String, color: String, perms: Vec<Perm>)
            -> Result<board::Board, sqlx::Error> {
        let mut id: Uuid;
        loop {
            id = Uuid::new_v4();
            if self.get_board(&id).await.is_none() { break; }
        }

        let created = chrono::offset::Utc::now();
        let edited = created.clone();

        sqlx::query(r#"INSERT INTO board.boards(id, name, description, creator_id, color, created, edited)
            VALUES($1, $2, $3, $4, $5, $6, $7);"#)
            .bind(id).bind(name).bind(desc).bind(creator_id).bind(color).bind(created).bind(edited)
            .execute(&self.pool).await?;

        let itr = perms.iter();
        sqlx::query(r#"BEGIN;"#).execute(&self.pool).await?;
        for val in itr {
            sqlx::query(r#"INSERT INTO board.board_perms(table_id, user_id, perm_id) VALUES($1, $2, $3);"#)
                .bind(id).bind(val.user_id.clone()).bind(val.perm_level.clone())
                .execute(&self.pool).await?;
        }
        sqlx::query(r#"COMMIT;"#).execute(&self.pool).await?;

        return Ok(self.get_board(&id).await.unwrap());
    }

    pub async fn modify_board(&mut self, board_id: &Uuid, name: Option<String>, desc: Option<String>,
        color: Option<String>, perms: Option<Vec<Perm>>)
            -> Result<board::Board, sqlx::Error> {
        // TODO

        return Ok(self.get_board(&board_id).await.unwrap());
    }

    // fn delete_board(&mut self, board_id: &Uuid) -> Result<(), &'static str>;

    // fn create_pin(&mut self, creator: &UserId, pin_type: pin::PinType, board_id: &Uuid, content: String,
    //     attachment_paths: Vec<String>, flags: u32, metadata: Value)
    //     -> Result<&pin::Pin, &'static str>;
    // fn modify_pin(&mut self, pin_id: &Uuid, pin_type: Option<pin::PinType>, board_id: &Option<Uuid>,
    //     content: Option<String>, attachment_paths: Option<Vec<String>>, flags: Option<u32>, metadata: Option<Value>)
    //     -> Result<&pin::Pin, &'static str>;
    // fn delete_pin(&mut self, pin_id: Uuid) -> Result<(), &'static str>;

    // fn get_boards(&self, user: &UserId, offset: Option<u32>, limit: Option<u32>, only_self: Option<bool>, not_self: Option<bool>,
    //     owner_search: &Option<String>, search_query: &Option<String>)
    //         -> Result<Vec<&board::Board>,  &'static str>;
    // fn get_pins(&self, offset: Option<u32>, limit: Option<u32>, search_query: &Option<String>)
    //     -> Result<Vec<&pin::Pin>, &'static str>;

    // TODO: get individual pin

    // TODO: get pin search globally
}
