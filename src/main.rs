use hellomouse_board_server::shared::handlers::postgres_handler::PostgresHandler;
use hellomouse_board_server::board::handlers::postgres_handler::PostgresHandler as BoardHandler;

use hellomouse_board_server::shared::types::account::{PermLevel, Perm};

#[tokio::main]
async fn main() -> Result<(), sqlx::Error> {
    let mut a = PostgresHandler::new().await?;
    let mut b = BoardHandler::new().await?;
    a.init().await?;
    b.init().await?;
    match a.create_account("bowserinator", "Bowser Inator", "12345").await {
        Ok(acc) => acc,
        Err(error) => println!("Account alread yexists")
    }
    // let valid = a.can_login("bowserinator", "12345").await?;
    // println!("Can login: {}", valid);
    // let valid = a.can_login("bowserinator", "1234566").await?;
    // println!("Can login: {}", valid);

    a.change_account_settings("bowserinator", serde_json::from_str("{\"age2\": 12345, \"key2\": 2}").unwrap()).await?;

    println!("{}", a.get_user("bowserinator").await?.settings);

    let mut perms = Vec::new();
    perms.push(Perm {
        user_id: "bowserinator".to_string(),
        perm_level: PermLevel::Owner
    });

    let r = b.create_board("My board".to_string(),
        "bowserinator",
        "a description".to_string(),
        "#FF0000".to_string(), perms).await?;
    println!("{}", r.perms[0].user_id);

    // a.delete_account("bowserinator").await?;
    Ok(())
}
