use actix_files;
use actix_multipart::Multipart;
use serde::{Deserialize, Serialize};

use crate::files::postgres_handler::PostgresHandler;
use crate::shared::types::app::{ErrorResponse, Response, login_fail};

use actix_identity::Identity;
use actix_web::{get, post, web::{self, Data}, HttpRequest, HttpResponse, Result};

#[derive(Serialize,Debug)]
struct FileUploadResponse {
    msg: String,
    failed_files: Vec<i8>
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
            Ok(failed_files) => Ok(HttpResponse::Ok().json(
                FileUploadResponse { msg: "File upload result".to_string(), failed_files })),
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
    id: String,
    owner: String
}

#[get("/v1/files/single")]
async fn get_file(handler: Data<PostgresHandler>, identity: Option<Identity>, body: web::Query<SingleFileSearch>, req: HttpRequest) -> Result<HttpResponse> {
    if identity.is_some() {
        let json_data = body.into_inner();
        let file_hash = json_data.id;
        let user_id = json_data.owner;
        let file_path = handler.file_exists(&user_id, &file_hash).await;

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
    login_fail!();
}

#[derive(Deserialize,Debug)]
struct FileSearch {
    offset: Option<i64>,
    limit: Option<i64>
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