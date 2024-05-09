use actix_identity::IdentityMiddleware;
use actix_session::{config::PersistentSession, storage::CookieSessionStore, SessionMiddleware};
use actix_web::{
    HttpResponse, web::{self, Data},
    cookie::time::Duration,
    middleware, App, HttpServer, Result
};
use actix_cors::Cors;
use actix_governor::{Governor, GovernorConfigBuilder};
use std::time;
use std::sync::Arc;

use crate::shared::util::config;
use crate::shared::util::secret;
use crate::shared::util::clean_html;

use crate::shared::handlers::postgres_handler::PostgresHandler as SharedPostgresHandler;
use crate::board::handlers::postgres_handler::PostgresHandler as BoardPostgresHandler;
use crate::link::handlers::postgres_handler::PostgresHandler as LinkPostgresHandler;
use crate::music::handlers::postgres_handler::PostgresHandler as MusicPostgresHandler;
use crate::site::handlers::web_handler::WebHandler as SiteWebHandler;
use crate::files::postgres_handler::PostgresHandler as FilesPostgresHandler;

use crate::shared::app as shared_app;
use crate::board::app as board_app;
use crate::site::app as site_app;
use crate::link::app as link_app;
use crate::music::app as music_app;
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
        .service(board_app::bulk_modify_board_colors)
        .service(board_app::get_boards)
        .service(board_app::get_board)
        .service(board_app::bulk_get_board_perms)
        .service(board_app::bulk_update_board_perms)
        
        // Files
        .service(files_app::create_file)
        .service(files_app::get_file)

        // Pins
        .service(board_app::create_pin)
        .service(board_app::modify_pin)
        .service(board_app::delete_pin)
        .service(board_app::get_pins)
        .service(board_app::bulk_delete_pins)
        .service(board_app::bulk_modify_pin_flags)
        .service(board_app::bulk_modify_pin_colors)
        .service(board_app::get_pin)
        .service(board_app::get_favorites)
        .service(board_app::add_favorites)
        .service(board_app::remove_favorites)
        .service(board_app::check_favorites)
        .service(board_app::get_pin_history_preview)
        .service(board_app::get_pin_history)
        
        // Board tags
        .service(board_app::get_tag)
        .service(board_app::get_tags)
        .service(board_app::create_tag)
        .service(board_app::modify_tag)
        .service(board_app::add_remove_board_tag)
        .service(board_app::bulk_modify_tag_colors)
        .service(board_app::move_board_tag)
        .service(board_app::delete_tags)
        
        // Link
        .service(link_app::add_link)
        .service(link_app::delete_link)
        .service(link_app::get_link)

        // Music
        .service(music_app::create_playlist)
        .service(music_app::edit_playlist)
        .service(music_app::edit_playlist_perms)
        .service(music_app::delete_playlist)
        .service(music_app::get_playlist)
        .service(music_app::get_playlists)
        .service(music_app::add_user_playlist)
        .service(music_app::remove_user_playlist)
        .service(music_app::add_songs_by_url)
        .service(music_app::get_song)
        .service(music_app::get_songs)

        // Site
        .service(site_app::get_pin_preview)
        .service(site_app::download_site)
        .service(site_app::job_status);
}

pub async fn start() -> std::io::Result<()> {
    if config::get_config().server.log {
        std::env::set_var("RUST_LOG", "debug");
        env_logger::init();
    }
    
    let secret_key = secret::get_session_key()?;
    let html_clean_rules = Arc::new(clean_html::get_html_rules());

    let handler1 = SharedPostgresHandler::new().await.unwrap();
    let handler2 = BoardPostgresHandler::new(html_clean_rules).await.unwrap();
    let handler3 = SiteWebHandler::new().await.unwrap();
    let handler4 = LinkPostgresHandler::new().await.unwrap();
    let handler5 = MusicPostgresHandler::new().await.unwrap();
    let handler6 = FilesPostgresHandler::new().await.unwrap();

    handler1.init().await.unwrap();
    handler2.init().await.unwrap();
    handler3.init().await.unwrap();
    handler4.init().await.unwrap();
    handler5.init().await.unwrap();
    handler6.init().await.unwrap();

    println!("starting HTTP server at http://localhost:{}", config::get_config().server.port);

    let governor_config = GovernorConfigBuilder::default()
        .per_millisecond(config::get_config().server.request_quota_replenish_ms)
        .burst_size(config::get_config().server.request_quota)
        .use_headers()
        .finish()
        .unwrap();

    HttpServer::new(move || {
        App::new()
            .app_data(Data::new(handler1.clone()))
            .app_data(Data::new(handler2.clone()))
            .app_data(Data::new(handler3.clone()))
            .app_data(Data::new(handler4.clone()))
            .app_data(Data::new(handler5.clone()))
            .app_data(Data::new(handler6.clone()))
            .configure(routes)
            .wrap(Governor::new(&governor_config))
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
