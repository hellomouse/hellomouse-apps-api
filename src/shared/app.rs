use crate::shared::handlers::postgres_handler::{PostgresHandler, UserSearchResult};
use crate::shared::types::app::{ErrorResponse, Response, login_fail};

use actix_identity::{Identity};
use actix_web::{
    get, post, put, HttpResponse, web::{self, Data},
    HttpMessage as _, HttpRequest, Result
};

use std::sync::Mutex;
use serde::{Serialize, Deserialize};
use serde_json::Value;


#[derive(Deserialize)]
struct LoginForm {
    username: String,
    password: String
}

#[derive(Serialize)]
struct LoginResponse {
    cookie: String
}

#[post("/v1/login")]
async fn login(handler: Data<Mutex<PostgresHandler>>, req: HttpRequest, info: web::Json<LoginForm>) -> Result<HttpResponse> {
    if !handler.lock().unwrap().can_login(info.username.as_str(), info.password.as_str()).await.unwrap() { login_fail!() }
    Identity::login(&req.extensions(), info.username.as_str().to_owned()).unwrap();
    
    // TODO: this line errors
    // req.cookie("login").unwrap().to_string()
    Ok(HttpResponse::Ok().json(LoginResponse{ cookie: "tet".to_string() }))
}

#[post("/v1/logout")]
async fn logout(id: Identity) -> Result<HttpResponse> {
    id.logout();
    Ok(HttpResponse::Ok().json(Response { msg: "You logged out".to_string() }))
}

#[derive(Deserialize)]
struct UserSettingsForm { settings: Value }

#[put("/v1/user_settings")]
async fn user_settings(handler: Data<Mutex<PostgresHandler>>, identity: Option<Identity>, params: web::Json<UserSettingsForm>)
        -> Result<HttpResponse> {
    if let Some(identity) = identity {
        return match handler.lock().unwrap()
            .change_account_settings(identity.id().unwrap().as_str(), params.settings.to_owned()).await {
            Ok(()) => Ok(HttpResponse::Ok().json(
                Response{ msg: "Settings updated".to_string() })),
            Err(_err) => Ok(HttpResponse::InternalServerError().json(
                ErrorResponse{ error: "Failed to update settings".to_string() }))
        };
    }
    login_fail!();
}

#[derive(Deserialize)]
struct UserSearchParams { filter: String }

#[derive(Serialize)]
struct UserSearchParamsReturn { users: Vec<UserSearchResult> }

#[get("/v1/users/search")]
async fn users_search(handler: Data<Mutex<PostgresHandler>>, identity: Option<Identity>, params: web::Query<UserSearchParams>) -> Result<HttpResponse> {
    if identity.is_some() {
        // Enforce filter is at least 2 characters long
        if params.filter.len() < 2 {
            return Ok(HttpResponse::Forbidden().json(
                ErrorResponse{ error: "Filter must be at least 3 characters long".to_string() }));
        }
        
        return match handler.lock().unwrap().search_users(params.filter.as_str()).await {
            Ok(result) => Ok(HttpResponse::Ok().json(UserSearchParamsReturn { users: result })),
            Err(_err) => Ok(HttpResponse::InternalServerError().json(ErrorResponse{ error: "Error in search".to_string() }))
        };
    }
    login_fail!();
}

#[derive(Deserialize)]
struct UserParams { id: String }

#[derive(Serialize)]
struct UserParamsReturn {
    name: String,
    id: String,
    pfp_url: String
}

#[get("/v1/users")]
async fn users(handler: Data<Mutex<PostgresHandler>>, identity: Option<Identity>, params: web::Query<UserParams>) -> Result<HttpResponse> {
    if identity.is_some() {
        return match handler.lock().unwrap().get_user(params.id.as_str()).await {
            Ok(user) => Ok(HttpResponse::Ok().json(UserParamsReturn {
                name: user.name,
                id: user.id,
                pfp_url: user.pfp_url
            })),
            Err(_err) => Ok(HttpResponse::Forbidden().json(ErrorResponse{ error: "Could not get user".to_string() }))
        };
    }
    login_fail!();
}

#[get("/v1/user_settings")]
async fn get_user_settings(handler: Data<Mutex<PostgresHandler>>, identity: Option<Identity>) -> Result<HttpResponse> {
    if let Some(identity) = identity {
        return match handler.lock().unwrap().get_user(identity.id().unwrap().as_str()).await {
            Ok(user) => Ok(HttpResponse::Ok().json(
                user.settings)),
            Err(_err) => Ok(HttpResponse::InternalServerError().json(
                ErrorResponse{ error: "Failed to get settings".to_string() }))
        };
    }
    login_fail!();
}
