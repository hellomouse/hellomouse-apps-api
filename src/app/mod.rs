use actix_identity::IdentityMiddleware;
use actix_session::{config::PersistentSession, storage::CookieSessionStore, SessionMiddleware};
use actix_web::{
    get, post, HttpResponse, web::{self, Data},
    cookie::{time::Duration, Key, SameSite},
    error,
    http::StatusCode,
    middleware, App, HttpMessage as _, HttpRequest, HttpServer, Responder, Result
};
use actix_cors::Cors;
use actix_extensible_rate_limit::{RateLimiter};
use actix_extensible_rate_limit::backend::{SimpleInputFunctionBuilder, memory::InMemoryBackend};
use std::time;

use crate::shared::util::config;

use crate::shared::handlers::postgres_handler::PostgresHandler as SharedPostgresHandler;
use crate::board::handlers::postgres_handler::PostgresHandler as BoardPostgresHandler;
use crate::files::postgres_handler::PostgresHandler as FilesPostgresHandler;

use crate::shared::app as shared_app;
use crate::board::app as board_app;
use crate::files::app as files_app;

use crate::shared::types::app as app_types;


async fn not_found() -> Result<HttpResponse> {
    let response = app_types::ErrorResponse{ error: "Resource not found".to_string() };
    Ok(HttpResponse::NotFound().json(response))
}

fn routes(app: &mut web::ServiceConfig) {
    app
        // User handling
        .service(shared_app::login)
        .service(shared_app::logout)
        .service(shared_app::user_settings)
        .service(shared_app::users)
        .service(shared_app::users_batch)
        .service(shared_app::users_search)
        .service(shared_app::get_user_settings)
        
        // Board
        .service(board_app::create_board)
        .service(board_app::update_board)
        .service(board_app::delete_board)
        .service(board_app::get_boards)
        .service(board_app::get_board)
        
        // Files
        .service(files_app::get_file)

        // Pins
        .service(board_app::create_pin)
        .service(board_app::modify_pin)
        .service(board_app::delete_pin)
        .service(board_app::get_pins)
        .service(board_app::bulk_delete_pins)
        .service(board_app::bulk_modify_pin_flags)
        .service(board_app::get_pin);
}

pub async fn start() -> std::io::Result<()> {
    if config::get_config().server.log {
        std::env::set_var("RUST_LOG", "debug");
        env_logger::init();
    }
    
    let secret_key = Key::generate(); // For sessions
    let backend = InMemoryBackend::builder().build(); // For rate limiting

    let handler1 = SharedPostgresHandler::new().await.unwrap();
    let handler2 = BoardPostgresHandler::new().await.unwrap();
    let handler3 = FilesPostgresHandler::new().await.unwrap();


    handler1.init().await.unwrap();
    handler2.init().await.unwrap();
    handler3.init().await.unwrap();

    println!("starting HTTP server at http://localhost:{}", config::get_config().server.port);

    HttpServer::new(move || {
        let input = SimpleInputFunctionBuilder::new(
                time::Duration::from_secs(config::get_config().server.max_requests_delta_seconds),
                config::get_config().server.max_requests_per_delta)
            .real_ip_key().build();
        // let rate_limit_middleware = RateLimiter::builder(backend.clone(), input).add_headers().build();

        App::new()
            .app_data(Data::new(handler1.clone()))
            .app_data(Data::new(handler2.clone()))
            .app_data(Data::new(handler3.clone()))
            .configure(routes)
            // .wrap(rate_limit_middleware)
            .wrap(IdentityMiddleware::default())
            .wrap(Cors::permissive()
                // .allowed_origin("http://localhost:3000")
                // .supports_credentials()
                // .allow_any_header()
                // .allowed_methods(vec!["GET", "POST", "PUT", "DELETE"])
            )
            .wrap(
                SessionMiddleware::builder(CookieSessionStore::default(), secret_key.clone())
                    .cookie_name("login".to_owned())
                    // .cookie_same_site(SameSite::Lax)
                    .cookie_secure(false)
                    .cookie_http_only(true)
                    .session_lifecycle(PersistentSession::default().session_ttl(Duration::seconds(
                        config::get_config().server.login_cookie_valid_duration_seconds.try_into().unwrap())))
                    .build(),
            )
            .wrap(middleware::NormalizePath::trim())
            .wrap(middleware::Logger::default())
            .default_service(web::route().to(not_found))
    })
        .keep_alive(time::Duration::from_secs(30))
        .bind(("127.0.0.1", config::get_config().server.port))?
        .run().await
}
