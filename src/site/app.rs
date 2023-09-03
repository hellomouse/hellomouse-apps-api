use crate::site::handlers::web_handler::WebHandler;
use crate::board::handlers::postgres_handler::PostgresHandler;
use crate::shared::types::app::{ErrorResponse, Response, login_fail, no_update_permission};
use crate::site::types::status::Job;

use actix_identity::Identity;
use actix_web::{
    get, post, put, HttpResponse, web::{self, Data},
    Result
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
        return Ok(HttpResponse::Ok().json(Response { msg: "Task queued".to_string() }));
    }
    login_fail!();
}


// Download a site
#[derive(Deserialize)]
struct SiteDownloadForm {
    url: String,
    strategy: String
}

#[derive(Serialize)]
struct UuidResponse {
    uuid: Uuid
}

#[post("/v1/site/download")]
async fn download_site(handler: Data<WebHandler>, identity: Option<Identity>, params: web::Json<SiteDownloadForm>) -> Result<HttpResponse> {
    if let Some(identity) = identity {
        let logged_in_id = identity.id().unwrap().to_owned();

        if params.strategy != "pdf" && params.strategy != "html" && params.strategy != "media" {
            return Ok(HttpResponse::Ok().json(ErrorResponse { error: "Unknown download strategy".to_string() }));
        }

        let result = handler.queue_site_download(params.strategy.as_str(), params.url.as_str(), &logged_in_id).await;
        if !result.is_ok() {
            return Ok(HttpResponse::Ok().json(ErrorResponse { error: "Failed to download site".to_string() }));
        }
        let result = result.unwrap();
        return Ok(HttpResponse::Ok().json(UuidResponse { uuid: result }));
    }
    login_fail!();
}


// Get job status
#[derive(Deserialize)]
struct SiteStatusForm {
    offset: Option<u32>,
    limit: Option<u32>
}

#[derive(Serialize)]
struct SiteStatusResponse {
    jobs: Vec<Job>
}

#[get("/v1/site/status")]
async fn job_status(handler: Data<WebHandler>, identity: Option<Identity>, params: web::Query<SiteStatusForm>) -> Result<HttpResponse> {
    if let Some(identity) = identity {
        let logged_in_id = identity.id().unwrap().to_owned();

        let result = handler.get_status_queue(&logged_in_id, params.offset.clone(), params.limit.clone()).await;
        if !result.is_ok() {
            return Ok(HttpResponse::Ok().json(ErrorResponse { error: "Failed to get status".to_string() }));
        }
        let result = result.unwrap();
        return Ok(HttpResponse::Ok().json(SiteStatusResponse { jobs: result }));
    }
    login_fail!();
}
