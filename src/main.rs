use hellomouse_board_server::shared::handlers::postgres_handler::PostgresHandler;

#[tokio::main]
async fn main() -> Result<(), sqlx::Error> {
    let mut a = PostgresHandler::new().await?;
    a.init().await?;
    match a.create_account("bowserinator", "Bowser Inator", "12345").await {
        Ok(acc) => acc,
        Err(error) => println!("Account alread yexists")
    }
    let valid = a.can_login("bowserinator", "12345").await?;
    println!("Can login: {}", valid);
    let valid = a.can_login("bowserinator", "1234566").await?;
    println!("Can login: {}", valid);

    a.change_account_settings("bowserinator", serde_json::from_str("{\"age2\": 12345, \"key2\": 2}").unwrap()).await?;

    println!("{}", a.get_user("bowserinator").await?.settings);

    // a.delete_account("bowserinator").await?;
    Ok(())
}
