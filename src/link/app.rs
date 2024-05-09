use crate::link::handlers::postgres_handler::PostgresHandler;
use crate::shared::handlers::postgres_handler::PostgresHandler as SharedPostgresHandler;
use crate::shared::types::app::{ErrorResponse, Response, login_fail};
use crate::link::types::Link;

use actix_identity::Identity;
use actix_web::{
    get, post, delete, HttpResponse, web::{self, Data}, Result
};
use serde::{Serialize, Deserialize};


// Append link
#[derive(Deserialize)]
struct AddLinkForm {
    url: String
}

#[derive(Serialize)]
struct AddLinkReturn { id: i32 }

#[post("/v1/link")]
async fn add_link(handler: Data<PostgresHandler>, identity: Option<Identity>, params: web::Json<AddLinkForm>) -> Result<HttpResponse> {
    if let Some(identity) = identity {
        return match handler.add_link(
            identity.id().unwrap().as_str(),
            params.url.as_str()
        ).await {
            Ok(result) => Ok(HttpResponse::Ok().json(AddLinkReturn { id: result })),
            Err(_err) => Ok(HttpResponse::InternalServerError().json(ErrorResponse{ error: "Error creating link".to_string() }))
        };
    }
    login_fail!();
}


// Delete link
#[derive(Deserialize)]
struct RemoveLinkForm {
    id: i32
}

#[delete("/v1/link")]
async fn delete_link(handler: Data<PostgresHandler>, identity: Option<Identity>, params: web::Json<RemoveLinkForm>) -> Result<HttpResponse> {
    if let Some(identity) = identity {
        return match handler.delete_link(
            identity.id().unwrap().as_str(),
            params.id
        ).await {
            Ok(_result) => Ok(HttpResponse::Ok().json(Response { msg: "Link removed".to_string() })),
            Err(_err) => Ok(HttpResponse::InternalServerError().json(ErrorResponse{ error: "Error deleting link".to_string() }))
        };
    }
    login_fail!();
}


// Get links
#[derive(Deserialize)]
struct GetLinkForm {
    user_id: String
}

#[derive(Serialize)]
struct GetLinkReturn {
    links: Vec<Link>,
    creator_name: String
}

#[get("/v1/link")]
async fn get_link(handler: Data<PostgresHandler>, user_handler: Data<SharedPostgresHandler>, params: web::Query<GetLinkForm>) -> Result<HttpResponse> {
    let user = user_handler.get_user(params.user_id.as_str()).await;
    let mut name = "".to_string();
    if user.is_ok() {
        name = user.unwrap().name;
    }

    return match handler.get_links(params.user_id.as_str()).await {
        Ok(result) => Ok(HttpResponse::Ok().json(GetLinkReturn {
            links: result,
            creator_name: name
        })),
        Err(_err) => Ok(HttpResponse::InternalServerError().json(ErrorResponse{ error: "Error getting links".to_string() }))
    };
}
