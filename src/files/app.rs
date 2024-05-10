use actix_files;
use actix_multipart::Multipart;
use serde::{Deserialize, Serialize};

use crate::files::postgres_handler::PostgresHandler;
use crate::shared::types::app::{ErrorResponse, Response, login_fail};

use actix_identity::Identity;
use actix_web::{get, post, delete, web::{self, Data}, HttpRequest, HttpResponse, Result};
use uuid::Uuid;

#[derive(Serialize,Debug)]
struct FileUploadResponse {
    msg: String,
    succeeded: Vec<String>,
    failed: Vec<u8>
}

#[post("/v1/files")]
async fn create_file(
    identity: Option<Identity>,
    handler: web::Data<PostgresHandler>,
    payload: Multipart,
) -> Result<HttpResponse> {
    if let Some(identity) = identity{
        let user_id = identity.id().unwrap();
        // Create the file using the handler
        let upload_status = handler.file_create(&user_id, payload).await;

        return match upload_status {
            Ok(result) => Ok(HttpResponse::Ok().json(
                FileUploadResponse {
                    msg: "File upload result".to_string(),
                    succeeded: result.succeeded,
                    failed: result.failed
                })),
            Err(e) => {
                eprintln!("Error: {:?}", e);
                return Ok(HttpResponse::Ok().json(
                    ErrorResponse { error: "File upload failed".to_string() }));
            }
        }
    }

    login_fail!();
}

#[derive(Deserialize,Debug)]
struct SingleFileSearch {
    id: Uuid
}

#[get("/v1/files/single")]
async fn get_file(handler: Data<PostgresHandler>, body: web::Query<SingleFileSearch>, req: HttpRequest) -> Result<HttpResponse> {
    let json_data = body.into_inner();
    let file_path = handler.file_exists(&json_data.id).await;

    match file_path {
        Ok(file_path) => {
            let file = actix_files::NamedFile::open_async(file_path).await.unwrap();
            return Ok(file.into_response(&req));
        },
        Err(_) => {
            return Ok(HttpResponse::Ok().json(ErrorResponse { error: "Not found".to_string() }))
        }
    };
}

#[delete("/v1/files/single")]
async fn delete_file(handler: Data<PostgresHandler>, identity: Option<Identity>, body: web::Json<SingleFileSearch>) -> Result<HttpResponse> {
    if let Some(identity) = identity {
        let logged_in_id = identity.id().unwrap().to_owned();
        let json_data = body.into_inner();
        let result = handler.delete_file(&logged_in_id, &json_data.id).await;

        match result {
            Ok(_) => { return Ok(HttpResponse::Ok().json(Response { msg: "Deleted".to_string() })); },
            Err(_) => { return Ok(HttpResponse::Ok().json(ErrorResponse { error: "Failed to delete".to_string() })) }
        };
    }
    login_fail!();
}

#[derive(Deserialize,Debug)]
struct FileSearch {
    offset: Option<u32>,
    limit: Option<u32>
}

#[get("/v1/files")]
async fn get_files(handler: Data<PostgresHandler>, identity: Option<Identity>, body: web::Query<FileSearch>) -> Result<HttpResponse> {
    if identity.is_some() {
        let json_data = body.into_inner();
        let offset = json_data.offset.unwrap_or(0);
        let limit = json_data.limit.unwrap_or(20);
        let user_id = identity.unwrap().id().unwrap();
        let files_result = handler.get_files(&user_id, offset, limit).await;

        match files_result {
            Ok(files) => {
                return Ok(HttpResponse::Ok().json(files));
            },
            Err(_) => {
                return Ok(HttpResponse::NotFound().json(ErrorResponse { error: "Not found".to_string() }));
            }
        }
    }
    login_fail!();
}