use crate::board::handlers::postgres_handler::PostgresHandler;
use crate::shared::types::app::{ErrorResponse, Response, login_fail, no_update_permission, no_view_permission};
use crate::shared::types::account::{Perm, PermLevel};
use crate::board::types::board::{SortBoard, Board, MassBoardShareUser};
use crate::board::types::pin::{PinFlags, Pin, SortPin, PinHistory, PinHistoryAbridged};

use actix_identity::Identity;
use actix_web::{
    get, post, put, delete, HttpResponse, web::{self, Data}, Result
};

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
    perms: HashMap<String, Perm>,
    tag_id: Option<i32>
}

#[derive(Serialize)]
struct ResponseWithId {
    id: Uuid
}

#[post("/v1/board/boards")]
async fn create_board(handler: Data<PostgresHandler>, identity: Option<Identity>, params: web::Json<CreateBoardForm>) -> Result<HttpResponse> {
    if let Some(identity) = identity {
        return match handler.create_board(
            params.name.as_str(),
            identity.id().unwrap().as_str(),
            params.desc.as_str(),
            params.color.as_str(),
            params.perms.clone(),
            params.tag_id.clone()
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
async fn update_board(handler: Data<PostgresHandler>, identity: Option<Identity>, params: web::Json<UpdateBoardForm>) -> Result<HttpResponse> {
    if let Some(identity) = identity {
        let board = handler.get_board(&params.id).await;
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
            return match handler.modify_board(
                &id_username,
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
async fn delete_board(handler: Data<PostgresHandler>, identity: Option<Identity>, params: web::Json<BoardIdForm>) -> Result<HttpResponse> {
    if let Some(identity) = identity {
        let board = handler.get_board(&params.id).await;
        if board.is_none() {
            return Ok(HttpResponse::Forbidden().json(ErrorResponse{ error: "Board ID does not exist".to_string() }));
        }
        let board = board.unwrap();
        let id_username = identity.id().unwrap();

        // Only owner can modify board
        if 
                board.perms.contains_key(&id_username) &&
                board.perms.get(&id_username).unwrap().perm_level == PermLevel::Owner {
            return match handler.delete_board(&params.id).await {
                Ok(_) => Ok(HttpResponse::Ok().json(Response { msg: "Deleted".to_string() })),
                Err(_err) => Ok(HttpResponse::InternalServerError().json(ErrorResponse{ error: "Error deleting board".to_string() }))
            };
        }

        no_update_permission!();
    }
    login_fail!();
}

// Bulk change board colors
#[derive(Deserialize)]
struct ModifyBoardColorsForm {
    board_ids: Vec<Uuid>,
    color: String
}

#[put("/v1/board/boards/bulk_colors")]
async fn bulk_modify_board_colors(handler: Data<PostgresHandler>, identity: Option<Identity>, params: web::Json<ModifyBoardColorsForm>) -> Result<HttpResponse> {
    if let Some(identity) = identity {
        let id_username = identity.id().unwrap();
        return match handler.mass_edit_board_colors(
                &id_username,
                params.board_ids.clone(),
                params.color.as_str()
            ).await {
            Ok(_) => Ok(HttpResponse::Ok().json(Response { msg: "Colors changed".to_string() })),
            Err(_err) => Ok(HttpResponse::InternalServerError().json(ErrorResponse{ error: "Error editing boards".to_string() }))
        };
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
    query: Option<String>,
    sort_by: Option<SortBoard>,
    sort_down: Option<bool>
}

#[derive(Serialize)]
struct SearchBoardReturn {
    boards: Vec<Board>
}

#[get("/v1/board/boards")]
async fn get_boards(handler: Data<PostgresHandler>, identity: Option<Identity>, params: web::Query<SearchBoardForm>) -> Result<HttpResponse> {
    if let Some(identity) = identity {
        let logged_in_id = identity.id().unwrap().to_owned();

        return match handler
            .get_boards(
                logged_in_id.as_str(),
                params.offset,
                params.limit,
                params.not_self,
                &params.owner_search,
                &params.query,
                params.sort_by.clone(),
                params.sort_down.clone()
            ).await {
                Ok(result) => Ok(HttpResponse::Ok().json(SearchBoardReturn { boards: result })),
                Err(_err) => Ok(HttpResponse::InternalServerError().json(
                    ErrorResponse{ error: "Failed to search for boards".to_string() }))
        };
    }
    login_fail!();
}

#[get("/v1/board/boards/single")]
async fn get_board(handler: Data<PostgresHandler>, identity: Option<Identity>, params: web::Query<BoardIdForm>) -> Result<HttpResponse> {
    let mut logged_in_id = "public".to_string();
    if let Some(identity) = identity {
        logged_in_id = identity.id().unwrap().to_owned();
    }
    
    // Having any permission means being able to view the board
    if handler.get_perms_for_board(logged_in_id.as_str(), &params.id).await.is_none() {
        no_update_permission!();
    }
    
    return match handler
        .get_board(&params.id).await {
            Some(result) => Ok(HttpResponse::Ok().json(result)),
            None => Ok(HttpResponse::InternalServerError().json(
                ErrorResponse{ error: "Failed to get board".to_string() }))
    };
}


// Get board perms
#[derive(Deserialize)]
struct GetBoardPermForm {
    board_ids: Vec<Uuid>
}

#[derive(Serialize)]
struct GetBoardPermReturn {
    perms: HashMap<String, MassBoardShareUser>
}

#[post("/v1/board/boards/perms/bulk")]
async fn bulk_get_board_perms(handler: Data<PostgresHandler>, identity: Option<Identity>, params: web::Json<GetBoardPermForm>) -> Result<HttpResponse> {
    if let Some(identity) = identity {
        let logged_in_id = identity.id().unwrap().to_owned();

        return match handler
            .get_mass_board_share_perms(
                logged_in_id.as_str(),
                &params.board_ids
            ).await {
                Ok(result) => Ok(HttpResponse::Ok().json(GetBoardPermReturn { perms: result })),
                Err(_err) => Ok(HttpResponse::InternalServerError().json(
                    ErrorResponse{ error: "Failed to get perms for boards".to_string() }))
        };
    }
    login_fail!();
}

// Get board perms
#[derive(Deserialize)]
struct UpdateBoardPermForm {
    board_ids: Vec<Uuid>,
    perms_to_add: HashMap<String, Perm>,
    users_to_delete: Vec<String>
}

#[derive(Serialize)]
struct UpdateBoardPermReturn {
    updated: i32
}

#[put("/v1/board/boards/perms/bulk")]
async fn bulk_update_board_perms(handler: Data<PostgresHandler>, identity: Option<Identity>, params: web::Json<UpdateBoardPermForm>) -> Result<HttpResponse> {
    if let Some(identity) = identity {
        let logged_in_id = identity.id().unwrap().to_owned();

        return match handler
            .mass_change_board_share_perms(
                logged_in_id.as_str(),
                &params.board_ids,
                &params.perms_to_add,
                &params.users_to_delete
            ).await {
                Ok(result) => Ok(HttpResponse::Ok().json(UpdateBoardPermReturn { updated: result })),
                Err(_err) => Ok(HttpResponse::InternalServerError().json(
                    ErrorResponse{ error: "Failed to update perms for boards".to_string() }))
        };
    }
    login_fail!();
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
async fn create_pin(handler: Data<PostgresHandler>, identity: Option<Identity>, params: web::Json<CreatePinForm>) -> Result<HttpResponse> {
    if let Some(identity) = identity {
        // SelfEdit, Edit, or Owner can create a pin
        let perm = handler.get_perms_for_board(identity.id().unwrap().to_owned().as_str(), &params.board_id).await;
        if perm.is_none() {
            no_update_permission!();
        }
        let perm = perm.unwrap().perm_level;
        if perm != PermLevel::Edit && perm != PermLevel::Owner && perm != PermLevel::SelfEdit {
            no_update_permission!();
        }

        return match handler.create_pin(
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
async fn modify_pin(handler: Data<PostgresHandler>, identity: Option<Identity>, params: web::Json<ModifyPinForm>) -> Result<HttpResponse> {
    if let Some(identity) = identity {
        if !handler.can_edit_pin(identity.id().unwrap().as_str(), &params.id).await {
            no_update_permission!();
        }

        let pin_type = match params.pin_type {
            Some(v) => Some(num::FromPrimitive::from_u32(v as u32).unwrap()),
            None => None
        };

        return match handler.modify_pin(
            identity.id().unwrap().as_str(),
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

// Bulk modify pin flags
#[derive(Deserialize)]
struct ModifyPinFlagsForm {
    pin_ids: Vec<Uuid>,
    new_flags: PinFlags,
    add_flags: bool
}

#[put("/v1/board/pins/bulk_flags")]
async fn bulk_modify_pin_flags(handler: Data<PostgresHandler>, identity: Option<Identity>, params: web::Json<ModifyPinFlagsForm>) -> Result<HttpResponse> {
    if let Some(identity) = identity {
        return match handler.mass_edit_pin_flags(
            identity.id().unwrap().as_str(),
            params.pin_ids.clone(),
            params.new_flags.clone(),
            params.add_flags
        ).await {
            Ok(()) => Ok(HttpResponse::Ok().json(Response { msg: "Updated pin flags".to_string() })),
            Err(_err) => Ok(HttpResponse::InternalServerError().json(ErrorResponse{ error: "Error updating pin".to_string() }))
        };
    }
    login_fail!();
}


// Bulk modify pin color
#[derive(Deserialize)]
struct ModifyPinColorsForm {
    pin_ids: Vec<Uuid>,
    color: String
}

#[put("/v1/board/pins/bulk_colors")]
async fn bulk_modify_pin_colors(handler: Data<PostgresHandler>, identity: Option<Identity>, params: web::Json<ModifyPinColorsForm>) -> Result<HttpResponse> {
    if let Some(identity) = identity {
        return match handler.mass_edit_pin_colors(
            identity.id().unwrap().as_str(),
            params.pin_ids.clone(),
            &params.color
        ).await {
            Ok(()) => Ok(HttpResponse::Ok().json(Response { msg: "Updated pin colors".to_string() })),
            Err(_err) => Ok(HttpResponse::InternalServerError().json(ErrorResponse{ error: "Error updating pin".to_string() }))
        };
    }
    login_fail!();
}

// Delete a pin
#[derive(Deserialize)]
struct PinIdForm { id: Uuid }

#[delete("/v1/board/pins")]
async fn delete_pin(handler: Data<PostgresHandler>, identity: Option<Identity>, params: web::Json<PinIdForm>) -> Result<HttpResponse> {
    if let Some(identity) = identity {
        let id_username = identity.id().unwrap();

        if handler.can_edit_pin(id_username.as_str(), &params.id).await {
            return match handler.delete_pin(&params.id).await {
                Ok(_) => Ok(HttpResponse::Ok().json(Response { msg: "Deleted".to_string() })),
                Err(_err) => Ok(HttpResponse::InternalServerError().json(ErrorResponse{ error: "Error updating pins".to_string() }))
            };
        }

        no_update_permission!();
    }
    login_fail!();
}

// Bulk delete pins
#[derive(Deserialize)]
struct BulkDeletePinsForm {
    pin_ids: Vec<Uuid>
}

#[delete("/v1/board/pins/bulk_delete")]
async fn bulk_delete_pins(handler: Data<PostgresHandler>, identity: Option<Identity>, params: web::Json<BulkDeletePinsForm>) -> Result<HttpResponse> {
    if let Some(identity) = identity {
        return match handler.mass_delete_pins(
            identity.id().unwrap().as_str(),
            params.pin_ids.clone()
        ).await {
            Ok(()) => Ok(HttpResponse::Ok().json(Response { msg: "Deleted pins".to_string() })),
            Err(_err) => Ok(HttpResponse::InternalServerError().json(ErrorResponse{ error: "Error deleting pins".to_string() }))
        };
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
    query: Option<String>,
    sort_by: Option<SortPin>,
    sort_down: Option<bool>
}

#[derive(Serialize)]
struct SearchPinReturn { pins: Vec<Pin> }

#[get("/v1/board/pins")]
async fn get_pins(handler: Data<PostgresHandler>, identity: Option<Identity>, params: web::Query<SearchPinForm>) -> Result<HttpResponse> {
    // Public user can get pins
    let mut logged_in_id = "public".to_string();
    if let Some(identity) = identity {
        logged_in_id = identity.id().unwrap().to_owned();
    }

    return match handler
        .get_pins(
            logged_in_id.as_str(),
            &params.board_id,
            params.offset,
            params.limit,
            &params.creator,
            &params.query,
            params.sort_by.clone(),
            params.sort_down.clone()
        ).await {
            Ok(result) => Ok(HttpResponse::Ok().json(SearchPinReturn { pins: result })),
            Err(_err) => Ok(HttpResponse::InternalServerError().json(
                ErrorResponse{ error: "Failed to search for pins".to_string() }))
    };
}

#[get("/v1/board/pins/single")]
async fn get_pin(handler: Data<PostgresHandler>, identity: Option<Identity>, params: web::Query<PinIdForm>) -> Result<HttpResponse> {
    let mut logged_in_id = "public".to_string();
    if let Some(identity) = identity {
        logged_in_id = identity.id().unwrap().to_owned();
    }

    // Having any permission means being able to view the pin
    if handler.get_perms_for_pin(logged_in_id.as_str(), &params.id).await.is_none() {
        no_update_permission!();
    }
    
    return match handler
        .get_pin(&params.id).await {
            Some(result) => Ok(HttpResponse::Ok().json(result)),
            None => Ok(HttpResponse::InternalServerError().json(
                ErrorResponse{ error: "Failed to get pin".to_string() }))
    };
}


// Add + delete favorites
#[derive(Deserialize)]
struct FavoritesForm {
    pin_ids: Vec<Uuid>
}

#[put("/v1/board/pins/favorites")]
async fn add_favorites(handler: Data<PostgresHandler>, identity: Option<Identity>, params: web::Json<FavoritesForm>) -> Result<HttpResponse> {
    if let Some(identity) = identity {
        return match handler.add_favorites(
            identity.id().unwrap().as_str(),
            &params.pin_ids
        ).await {
            Ok(()) => Ok(HttpResponse::Ok().json(Response { msg: "Added favorites".to_string() })),
            Err(_err) => Ok(HttpResponse::InternalServerError().json(ErrorResponse{ error: "Error adding pins".to_string() }))
        };
    }
    login_fail!();
}

#[delete("/v1/board/pins/favorites")]
async fn remove_favorites(handler: Data<PostgresHandler>, identity: Option<Identity>, params: web::Json<FavoritesForm>) -> Result<HttpResponse> {
    if let Some(identity) = identity {
        return match handler.remove_favorites(
            identity.id().unwrap().as_str(),
            &params.pin_ids
        ).await {
            Ok(()) => Ok(HttpResponse::Ok().json(Response { msg: "Deleted favorites".to_string() })),
            Err(_err) => Ok(HttpResponse::InternalServerError().json(ErrorResponse{ error: "Error deleting pins".to_string() }))
        };
    }
    login_fail!();
}

// Get favorites
#[derive(Deserialize)]
struct SearchFavoritesForm {
    offset: Option<u32>,
    limit: Option<u32>,
    sort_by: Option<SortPin>,
    sort_down: Option<bool>
}

#[get("/v1/board/pins/favorites")]
async fn get_favorites(handler: Data<PostgresHandler>, identity: Option<Identity>, params: web::Query<SearchFavoritesForm>) -> Result<HttpResponse> {
    if let Some(identity) = identity {
        return match handler.get_favorites(
            identity.id().unwrap().as_str(),
            params.offset.clone(),
            params.limit.clone(),
            params.sort_by.clone(),
            params.sort_down.clone()
        ).await {
            Ok(result) => Ok(HttpResponse::Ok().json(SearchPinReturn { pins: result })),
            Err(_err) => Ok(HttpResponse::InternalServerError().json(ErrorResponse{ error: "Failed to get favorites".to_string() }))
        };
    }
    login_fail!();
}

#[derive(Serialize)]
struct CheckFavoriteReturn { pins: Vec<Uuid> }

#[post("/v1/board/pins/favorites/check")]
async fn check_favorites(handler: Data<PostgresHandler>, identity: Option<Identity>, params: web::Json<FavoritesForm>) -> Result<HttpResponse> {
    if let Some(identity) = identity {
        return match handler.check_favorites(
            identity.id().unwrap().as_str(),
            &params.pin_ids
        ).await {
            Ok(result) => Ok(HttpResponse::Ok().json(CheckFavoriteReturn { pins: result })),
            Err(_err) => Ok(HttpResponse::InternalServerError().json(ErrorResponse{ error: "Failed to check favorites".to_string() }))
        };
    }
    login_fail!();
}




// Pin history preview
// -------------------------

#[derive(Deserialize)]
struct PinHistoryPreviewForm {
    pin_id: Uuid
}

#[derive(Serialize)]
struct PinHistoryPreviewReturnForm {
    history: Vec<PinHistoryAbridged>
}

#[derive(Deserialize)]
struct PinHistoryForm {
    pin_id: Uuid,
    history_id: i32
}

#[derive(Serialize)]
struct PinHistoryReturnForm {
    history: Option<PinHistory>
}

#[get("/v1/board/pins/history/preview")]
async fn get_pin_history_preview(handler: Data<PostgresHandler>, identity: Option<Identity>, params: web::Query<PinHistoryPreviewForm>) -> Result<HttpResponse> {
    let mut logged_in_id = "public".to_string();
    if let Some(identity) = identity {
        logged_in_id = identity.id().unwrap().to_owned();
    }

    let result = handler.get_pin_history_preview(&params.pin_id, logged_in_id.as_str()).await;
    if !result.is_ok() {
        return Ok(HttpResponse::Ok().json(ErrorResponse { error: "Failed to fetch history preview".to_string() }));
    }
    return Ok(HttpResponse::Ok().json(PinHistoryPreviewReturnForm { history: result.unwrap() }));
}

#[get("/v1/board/pins/history")]
async fn get_pin_history(handler: Data<PostgresHandler>, identity: Option<Identity>, params: web::Query<PinHistoryForm>) -> Result<HttpResponse> {
    let mut logged_in_id = "public".to_string();
    if let Some(identity) = identity {
        logged_in_id = identity.id().unwrap().to_owned();
    }

    let result = handler.get_pin_history(&params.pin_id, params.history_id, logged_in_id.as_str()).await;
    if !result.is_ok() {
        return Ok(HttpResponse::Ok().json(ErrorResponse { error: "Failed to fetch history".to_string() }));
    }
    return Ok(HttpResponse::Ok().json(PinHistoryReturnForm { history: result.unwrap() }));
}





// Tags
// -------------------------

#[derive(Clone, Serialize, Deserialize)]
pub struct TagAbridged {
    pub name: String,
    pub id: i32,
    pub color: String,
    pub board_ids: Vec<Uuid>
}


#[derive(Deserialize)]
struct TagWithIdForm {
    id: i32
}

#[derive(Deserialize)]
struct GetTagsForm {}

#[derive(Serialize)]
struct TagReturnForm {
    tag: TagAbridged
}

#[derive(Serialize)]
struct TagsReturnForm {
    tags: Vec<TagAbridged>
}

#[get("/v1/board/tags/single")]
async fn get_tag(handler: Data<PostgresHandler>, identity: Option<Identity>, params: web::Query<TagWithIdForm>) -> Result<HttpResponse> {
    if let Some(identity) = identity {
        let logged_in_id = identity.id().unwrap().to_owned();
        return match handler.get_tag(logged_in_id.as_str(), params.id).await {
            Ok(result) => Ok(HttpResponse::Ok().json(TagReturnForm { tag: TagAbridged {
                name: result.name,
                id: result.id,
                color: result.color,
                board_ids: result.board_ids
            }})),
            Err(_err) => Ok(HttpResponse::InternalServerError().json(ErrorResponse{ error: "Error getting tag".to_string() }))
        };
    }
    login_fail!();
}

#[get("/v1/board/tags")]
async fn get_tags(handler: Data<PostgresHandler>, identity: Option<Identity>, _params: web::Query<GetTagsForm>) -> Result<HttpResponse> {
    if let Some(identity) = identity {
        let logged_in_id = identity.id().unwrap().to_owned();
        return match handler.get_tags(logged_in_id.as_str()).await {
            Ok(result) => Ok(HttpResponse::Ok().json(TagsReturnForm {
                tags: result.into_iter()
                    .map(|r| TagAbridged {
                        name: r.name,
                        id: r.id,
                        color: r.color,
                        board_ids: r.board_ids
                    })
                    .collect::<Vec<TagAbridged>>()
            })),
            Err(_err) => Ok(HttpResponse::InternalServerError().json(ErrorResponse{ error: "Error getting tag".to_string() }))
        };
    }
    login_fail!();
}

#[derive(Deserialize)]
struct CreateTagForm {
    name: String,
    color: String
}

#[post("/v1/board/tags")]
async fn create_tag(handler: Data<PostgresHandler>, identity: Option<Identity>, params: web::Json<CreateTagForm>) -> Result<HttpResponse> {
    if let Some(identity) = identity {
        let logged_in_id = identity.id().unwrap().to_owned();
        return match handler.create_tag(logged_in_id.as_str(), params.name.as_str(), params.color.as_str()).await {
            Ok(_result) => Ok(HttpResponse::Ok().json(Response { msg: "Tag created".to_string() })),
            Err(_err) => Ok(HttpResponse::InternalServerError().json(ErrorResponse{ error: "Error creating tag".to_string() }))
        };
    }
    login_fail!();
}

#[derive(Deserialize)]
struct ModifyTagForm {
    id: i32,
    name: Option<String>,
    color: Option<String>,
    board_ids: Option<Vec<Uuid>>
}

#[put("/v1/board/tags")]
async fn modify_tag(handler: Data<PostgresHandler>, identity: Option<Identity>, params: web::Json<ModifyTagForm>) -> Result<HttpResponse> {
    if let Some(identity) = identity {
        let logged_in_id = identity.id().unwrap().to_owned();
        return match handler.modify_tag(
                logged_in_id.as_str(),
                params.id,
                params.name.clone(),
                params.color.clone(),
                params.board_ids.clone()
            ).await {
            Ok(_result) => Ok(HttpResponse::Ok().json(Response { msg: "Tag updated".to_string() })),
            Err(_err) => Ok(HttpResponse::InternalServerError().json(ErrorResponse{ error: "Error updating tag".to_string() }))
        };
    }
    login_fail!();
}

#[derive(Deserialize)]
struct AddRemoveBoardTagForm {
    id: i32,
    board_ids_to_delete: Vec<Uuid>,
    board_ids_to_add: Vec<Uuid>
}

#[put("/v1/board/tags/boards")]
async fn add_remove_board_tag(handler: Data<PostgresHandler>, identity: Option<Identity>, params: web::Json<AddRemoveBoardTagForm>) -> Result<HttpResponse> {
    if let Some(identity) = identity {
        let logged_in_id = identity.id().unwrap().to_owned();
        return match handler.tag_add_remove_boards(
                logged_in_id.as_str(),
                params.id,
                &params.board_ids_to_add,
                &params.board_ids_to_delete
            ).await {
            Ok(_result) => Ok(HttpResponse::Ok().json(Response { msg: "Tag updated".to_string() })),
            Err(_err) => Ok(HttpResponse::InternalServerError().json(ErrorResponse{ error: "Error updating tag".to_string() }))
        };
    }
    login_fail!();
}


// Bulk change board colors
#[derive(Deserialize)]
struct BulkModifyTagColorsForm {
    tag_ids: Vec<i32>,
    color: String
}

#[put("/v1/board/tags/bulk_colors")]
async fn bulk_modify_tag_colors(handler: Data<PostgresHandler>, identity: Option<Identity>, params: web::Json<BulkModifyTagColorsForm>) -> Result<HttpResponse> {
    if let Some(identity) = identity {
        let id_username = identity.id().unwrap();
        return match handler.mass_edit_tag_colors(
                &id_username,
                &params.tag_ids,
                &params.color
            ).await {
            Ok(_) => Ok(HttpResponse::Ok().json(Response { msg: "Colors changed".to_string() })),
            Err(_err) => Ok(HttpResponse::InternalServerError().json(ErrorResponse{ error: "Error editing tags".to_string() }))
        };
    }
    login_fail!();
}

#[derive(Deserialize)]
struct DeleteTagsForm {
    ids: Vec<i32>
}

#[delete("/v1/board/tags")]
async fn delete_tags(handler: Data<PostgresHandler>, identity: Option<Identity>, params: web::Json<DeleteTagsForm>) -> Result<HttpResponse> {
    if let Some(identity) = identity {
        let logged_in_id = identity.id().unwrap().to_owned();
        return match handler.delete_tags(logged_in_id.as_str(), &params.ids).await {
            Ok(_result) => Ok(HttpResponse::Ok().json(Response { msg: "Tag deleted".to_string() })),
            Err(_err) => Ok(HttpResponse::InternalServerError().json(ErrorResponse{ error: "Error deleting tag".to_string() }))
        };
    }
    login_fail!();
}

#[derive(Deserialize)]
struct MoveBoardTagForm {
    to_tag_id: i32,
    board_id: Uuid
}

#[post("/v1/board/tags/move")]
async fn move_board_tag(handler: Data<PostgresHandler>, identity: Option<Identity>, params: web::Json<MoveBoardTagForm>) -> Result<HttpResponse> {
    if let Some(identity) = identity {
        let logged_in_id = identity.id().unwrap().to_owned();
        return match handler.move_board_tag(logged_in_id.as_str(), &params.board_id, params.to_tag_id).await {
            Ok(_result) => Ok(HttpResponse::Ok().json(Response { msg: "Board moved".to_string() })),
            Err(_err) => Ok(HttpResponse::InternalServerError().json(ErrorResponse{ error: "Error moving board".to_string() }))
        };
    }
    login_fail!();
}
