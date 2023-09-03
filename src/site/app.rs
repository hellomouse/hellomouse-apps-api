use crate::site::handlers::web_handler::WebHandler;
use crate::board::handlers::postgres_handler::PostgresHandler;
use crate::shared::types::app::{ErrorResponse, Response, login_fail, no_update_permission};

use actix_identity::Identity;
use actix_web::{
    get, post, put, HttpResponse, web::{self, Data},
    HttpMessage as _, HttpRequest, Result
};

use serde::{Serialize, Deserialize};
use uuid::Uuid;


#[derive(Deserialize)]
struct SitePreviewForm {
    url: String,
    pin_id: Uuid
}

#[post("/v1/board/pins/preview")]
async fn get_pin_preview(board_handler: Data<PostgresHandler>, handler: Data<WebHandler>, identity: Option<Identity>, params: web::Json<SitePreviewForm>) -> Result<HttpResponse> {
    if let Some(identity) = identity {
        let logged_in_id = identity.id().unwrap().to_owned();
        if !board_handler.can_edit_pin(logged_in_id.as_str(), &params.pin_id).await {
            no_update_permission!();
        }

        let result = handler.get_preview(&params.pin_id, params.url.as_str(), &logged_in_id).await;
        if !result.is_ok() {
            return Ok(HttpResponse::Ok().json(ErrorResponse { error: "Failed to fetch preview".to_string() }));
        }
        let result = result.unwrap();
        return Ok(HttpResponse::Ok().json(result));
    }
    login_fail!();
}
