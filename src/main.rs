use hellomouse_board_server::app;

#[actix_web::main]
async fn main() -> Result<(), sqlx::Error> {
    let _r = app::start().await;
    Ok(())
}
