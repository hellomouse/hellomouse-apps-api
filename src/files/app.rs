use actix_files;
use serde::Deserialize;

use crate::files::postgres_handler::PostgresHandler;
use crate::shared::types::app::{ErrorResponse, Response, login_fail};

use actix_identity::Identity;
use actix_web::{
    get, post, put, delete, HttpResponse, web::{self, Data}, HttpRequest, Result
};




// Create a serde struct to parse json data
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
                return Ok(HttpResponse::Ok().json(Response { msg: "Not found".to_string() }))
            }

        };
    }
    login_fail!();

}


