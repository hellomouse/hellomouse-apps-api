use crate::board::handlers::postgres_handler::{PostgresHandler};
use crate::shared::types::app::{ErrorResponse, Response, login_fail, no_update_permission, no_view_permission};
use crate::shared::types::account::{Perm, PermLevel, Account};
use crate::board::types::board::Board;
use crate::board::types::pin::{PinFlags, PinType, Pin};

use actix_identity::{Identity};
use actix_web::{
    get, post, put, delete, HttpResponse, web::{self, Data},
    HttpMessage as _, HttpRequest, Result
};

use std::sync::Mutex;
use std::collections::HashMap;
use uuid::Uuid;

use serde::{Serialize, Deserialize};
use serde_json::Value;


// Create a new board
#[derive(Deserialize)]
struct CreateBoardForm {
    name: String,
    desc: String,
    color: String,
    perms: HashMap<String, Perm>
}

#[derive(Serialize)]
struct ResponseWithId {
    id: Uuid
}

#[post("/v1/board/boards")]
async fn create_board(handler: Data<Mutex<PostgresHandler>>, identity: Option<Identity>, params: web::Json<CreateBoardForm>) -> Result<HttpResponse> {
    if let Some(identity) = identity {
        return match handler.lock().unwrap().create_board(
            params.name.clone(),
            identity.id().unwrap().as_str(),
            params.desc.clone(),
            params.color.clone(),
            params.perms.clone()
        ).await {
            Ok(result) => Ok(HttpResponse::Ok().json(ResponseWithId { id: result.id })),
            Err(_err) => Ok(HttpResponse::InternalServerError().json(ErrorResponse{ error: "Error creating board".to_string() }))
        };
    }
    login_fail!();
}

// Update a board
#[derive(Deserialize)]
struct UpdateBoardForm {
    id: Uuid,
    name: Option<String>,
    desc: Option<String>,
    color: Option<String>,
    perms: Option<HashMap<String, Perm>>
}

#[put("/v1/board/boards")]
async fn update_board(handler: Data<Mutex<PostgresHandler>>, identity: Option<Identity>, params: web::Json<UpdateBoardForm>) -> Result<HttpResponse> {
    if let Some(identity) = identity {
        let board = handler.lock().unwrap().get_board(&params.id).await;
        if board.is_none() {
            return Ok(HttpResponse::Forbidden().json(ErrorResponse{ error: "Board ID does not exist".to_string() }));
        }
        let board = board.unwrap();
        let id_username = identity.id().unwrap();

        // Only owner or editor can modify board
        if 
                board.perms.contains_key(&id_username) &&
                (board.perms.get(&id_username).unwrap().perm_level == PermLevel::Owner ||
                 board.perms.get(&id_username).unwrap().perm_level == PermLevel::Edit) {
            return match handler.lock().unwrap().modify_board(
                id_username,
                &params.id,
                params.name.clone(),
                params.desc.clone(),
                params.color.clone(),
                params.perms.clone()
            ).await {
                Ok(result) => Ok(HttpResponse::Ok().json(ResponseWithId { id: result.id })),
                Err(_err) => Ok(HttpResponse::InternalServerError().json(ErrorResponse{ error: "Error updating board".to_string() }))
            };
        }

        no_update_permission!();
    }
    login_fail!();
}

// Delete a board
#[derive(Deserialize)]
struct BoardIdForm { id: Uuid }

#[delete("/v1/board/boards")]
async fn delete_board(handler: Data<Mutex<PostgresHandler>>, identity: Option<Identity>, params: web::Json<BoardIdForm>) -> Result<HttpResponse> {
    if let Some(identity) = identity {
        let board = handler.lock().unwrap().get_board(&params.id).await;
        if board.is_none() {
            return Ok(HttpResponse::Forbidden().json(ErrorResponse{ error: "Board ID does not exist".to_string() }));
        }
        let board = board.unwrap();
        let id_username = identity.id().unwrap();

        // Only owner can modify board
        if 
                board.perms.contains_key(&id_username) &&
                board.perms.get(&id_username).unwrap().perm_level == PermLevel::Owner {
            return match handler.lock().unwrap().delete_board(&params.id).await {
                Ok(_) => Ok(HttpResponse::Ok().json(Response { msg: "Deleted".to_string() })),
                Err(_err) => Ok(HttpResponse::InternalServerError().json(ErrorResponse{ error: "Error deleting board".to_string() }))
            };
        }

        no_update_permission!();
    }
    login_fail!();
}

// Get boards
#[derive(Deserialize)]
struct SearchBoardForm {
    offset: Option<u32>,
    limit: Option<u32>,
    not_self: Option<bool>,
    owner_search: Option<String>,
    query: Option<String>
}

#[derive(Serialize)]
struct SearchBoardReturn {
    boards: Vec<Board>
}

#[get("/v1/board/boards")]
async fn get_boards(handler: Data<Mutex<PostgresHandler>>, identity: Option<Identity>, params: web::Query<SearchBoardForm>) -> Result<HttpResponse> {
    // Public user can get boards
    let mut logged_in_id = "public".to_string();
    if let Some(identity) = identity {
        logged_in_id = identity.id().unwrap().to_owned();
    }

    return match handler.lock().unwrap()
        .get_boards(
            logged_in_id.as_str(),
            params.offset,
            params.limit,
            params.not_self,
            &params.owner_search,
            &params.query
        ).await {
            Ok(result) => Ok(HttpResponse::Ok().json(SearchBoardReturn { boards: result })),
            Err(_err) => Ok(HttpResponse::InternalServerError().json(
                ErrorResponse{ error: "Failed to search for boards".to_string() }))
    };
}

#[get("/v1/board/boards/single")]
async fn get_board(handler: Data<Mutex<PostgresHandler>>, identity: Option<Identity>, params: web::Query<BoardIdForm>) -> Result<HttpResponse> {
    let mut logged_in_id = "public".to_string();
    if let Some(identity) = identity {
        logged_in_id = identity.id().unwrap().to_owned();
    }
    
    // Having any permission means being able to view the board
    if handler.lock().unwrap().get_perms_for_board(logged_in_id.as_str(), &params.id).await.is_none() {
        no_update_permission!();
    }
    
    return match handler.lock().unwrap()
        .get_board(&params.id).await {
            Some(result) => Ok(HttpResponse::Ok().json(result)),
            None => Ok(HttpResponse::InternalServerError().json(
                ErrorResponse{ error: "Failed to get board".to_string() }))
    };
}



// ------------------- Pins ---------------------

// Create a pin
#[derive(Deserialize)]
struct CreatePinForm {
    pin_type: i32,
    flags: PinFlags,
    board_id: Uuid,
    content: String,
    attachment_paths: Vec<String>,
    metadata: Value
}

#[post("/v1/board/pins")]
async fn create_pin(handler: Data<Mutex<PostgresHandler>>, identity: Option<Identity>, params: web::Json<CreatePinForm>) -> Result<HttpResponse> {
    if let Some(identity) = identity {
        // SelfEdit, Edit, or Owner can create a pin
        let perm = handler.lock().unwrap().get_perms_for_board(identity.id().unwrap().to_owned().as_str(), &params.board_id).await;
        if perm.is_none() {
            no_update_permission!();
        }
        let perm = perm.unwrap().perm_level;
        if perm != PermLevel::Edit && perm != PermLevel::Owner && perm != PermLevel::SelfEdit {
            no_update_permission!();
        }

        return match handler.lock().unwrap().create_pin(
            identity.id().unwrap().as_str(),
            num::FromPrimitive::from_u32(params.pin_type as u32).unwrap(),
            &params.board_id,
            params.content.clone(),
            params.attachment_paths.clone(),
            params.flags.clone(),
            params.metadata.clone()
        ).await {
            Ok(result) => Ok(HttpResponse::Ok().json(ResponseWithId { id: result.pin_id })),
            Err(_err) => Ok(HttpResponse::InternalServerError().json(ErrorResponse{ error: "Error creating pin".to_string() }))
        };
    }
    login_fail!();
}

// Modify a pin
#[derive(Deserialize)]
struct ModifyPinForm {
    id: Uuid,
    pin_type: Option<i32>,
    flags: Option<PinFlags>,
    board_id: Option<Uuid>,
    content: Option<String>,
    attachment_paths: Option<Vec<String>>,
    metadata: Option<Value>
}

#[put("/v1/board/pins")]
async fn modify_pin(handler: Data<Mutex<PostgresHandler>>, identity: Option<Identity>, params: web::Json<ModifyPinForm>) -> Result<HttpResponse> {
    if let Some(identity) = identity {
        if !handler.lock().unwrap().can_edit_pin(identity.id().unwrap().as_str(), &params.id).await {
            no_update_permission!();
        }

        let pin_type = match params.pin_type {
            Some(v) => Some(num::FromPrimitive::from_u32(v as u32).unwrap()),
            None => None
        };

        return match handler.lock().unwrap().modify_pin(
            &params.id,
            pin_type,
            &params.board_id,
            params.content.clone(),
            params.attachment_paths.clone(),
            params.flags.clone(),
            params.metadata.clone()
        ).await {
            Ok(result) => Ok(HttpResponse::Ok().json(ResponseWithId { id: result.pin_id })),
            Err(_err) => Ok(HttpResponse::InternalServerError().json(ErrorResponse{ error: "Error updating pin".to_string() }))
        };
    }
    login_fail!();
}

// Delete a pin
#[derive(Deserialize)]
struct PinIdForm { id: Uuid }

#[delete("/v1/board/pins")]
async fn delete_pin(handler: Data<Mutex<PostgresHandler>>, identity: Option<Identity>, params: web::Json<PinIdForm>) -> Result<HttpResponse> {
    if let Some(identity) = identity {
        let id_username = identity.id().unwrap();

        if handler.lock().unwrap().can_edit_pin(id_username.as_str(), &params.id).await {
            return match handler.lock().unwrap().delete_pin(&params.id).await {
                Ok(_) => Ok(HttpResponse::Ok().json(Response { msg: "Deleted".to_string() })),
                Err(_err) => Ok(HttpResponse::InternalServerError().json(ErrorResponse{ error: "Error updating pin".to_string() }))
            };
        }

        no_update_permission!();
    }
    login_fail!();
}


// Get pins
#[derive(Deserialize)]
struct SearchPinForm {
    board_id: Option<Uuid>,
    offset: Option<u32>,
    limit: Option<u32>,
    creator: Option<String>,
    query: Option<String>
}

#[derive(Serialize)]
struct SearchPinReturn { pins: Vec<Pin> }

#[get("/v1/board/pins")]
async fn get_pins(handler: Data<Mutex<PostgresHandler>>, identity: Option<Identity>, params: web::Query<SearchPinForm>) -> Result<HttpResponse> {
    // Public user can get pins
    let mut logged_in_id = "public".to_string();
    if let Some(identity) = identity {
        logged_in_id = identity.id().unwrap().to_owned();
    }

    return match handler.lock().unwrap()
        .get_pins(
            logged_in_id.as_str(),
            &params.board_id,
            params.offset,
            params.limit,
            &params.creator,
            &params.query
        ).await {
            Ok(result) => Ok(HttpResponse::Ok().json(SearchPinReturn { pins: result })),
            Err(_err) => Ok(HttpResponse::InternalServerError().json(
                ErrorResponse{ error: _err.to_string() })) // "Failed to search for pins"
    };
}

#[get("/v1/board/pins/single")]
async fn get_pin(handler: Data<Mutex<PostgresHandler>>, identity: Option<Identity>, params: web::Query<PinIdForm>) -> Result<HttpResponse> {
    let mut logged_in_id = "public".to_string();
    if let Some(identity) = identity {
        logged_in_id = identity.id().unwrap().to_owned();
    }

    // Having any permission means being able to view the pin
    if handler.lock().unwrap().get_perms_for_pin(logged_in_id.as_str(), &params.id).await.is_none() {
        no_update_permission!();
    }
    
    return match handler.lock().unwrap()
        .get_pin(&params.id).await {
            Some(result) => Ok(HttpResponse::Ok().json(result)),
            None => Ok(HttpResponse::InternalServerError().json(
                ErrorResponse{ error: "Failed to get pin".to_string() }))
    };
}
