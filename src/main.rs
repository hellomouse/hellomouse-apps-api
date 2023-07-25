use hellomouse_board_server::shared::handlers::postgres_handler::PostgresHandler;
use hellomouse_board_server::board::handlers::postgres_handler::PostgresHandler as BoardHandler;

use hellomouse_board_server::shared::types::account::{PermLevel, Perm};

use hellomouse_board_server::app;

#[actix_web::main]
async fn main() -> Result<(), sqlx::Error> {
    app::start().await;
    Ok(())
}
