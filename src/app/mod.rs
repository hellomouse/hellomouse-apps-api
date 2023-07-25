use actix_identity::IdentityMiddleware;
use actix_session::{config::PersistentSession, storage::CookieSessionStore, SessionMiddleware};
use actix_web::{
    get, post, HttpResponse, web::{self, Data},
    cookie::{time::Duration, Key},
    error,
    http::StatusCode,
    middleware, App, HttpMessage as _, HttpRequest, HttpServer, Responder, Result
};

use std::sync::Mutex;

use crate::shared::util::config;

use crate::shared::handlers::postgres_handler::PostgresHandler as SharedPostgresHandler;
use crate::board::handlers::postgres_handler::PostgresHandler as BoardPostgresHandler;

use crate::shared::app as shared_app;
use crate::board::app as board_app;

use crate::shared::types::app as app_types;


const ONE_MINUTE: Duration = Duration::minutes(320);


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
        .service(shared_app::users_search)
        .service(shared_app::get_user_settings)
        
        // Board
        .service(board_app::create_board)
        .service(board_app::update_board)
        .service(board_app::delete_board)
        .service(board_app::get_boards)
        .service(board_app::get_board)
        
        // Pins
        .service(board_app::create_pin)
        .service(board_app::modify_pin)
        .service(board_app::delete_pin)
        .service(board_app::get_pins)
        .service(board_app::get_pin);
}

pub async fn start() -> std::io::Result<()> {
    std::env::set_var("RUST_LOG", "debug");
    env_logger::init();
    
    let secret_key = Key::generate();

    let mut handler1 = BoardPostgresHandler::new().await.unwrap();
    let mut handler2 = SharedPostgresHandler::new().await.unwrap();

    handler1.init().await.unwrap();
    handler2.init().await.unwrap();

    println!("starting HTTP server at http://localhost:{}", config::get_config().server.port);

    HttpServer::new(move || App::new()
        .app_data(Data::new(Mutex::new(handler1.clone())))
        .app_data(Data::new(Mutex::new(handler2.clone())))
        .configure(routes)
        .wrap(IdentityMiddleware::default())
        .wrap(
            SessionMiddleware::builder(CookieSessionStore::default(), secret_key.clone())
                .cookie_name("login".to_owned())
                .cookie_secure(false)
                .session_lifecycle(PersistentSession::default().session_ttl(ONE_MINUTE))
                .build(),
        )
        .wrap(middleware::NormalizePath::trim())
        .wrap(middleware::Logger::default())
        .default_service(web::route().to(not_found))
    )
        .bind(("127.0.0.1", config::get_config().server.port))?
        .run().await
}
