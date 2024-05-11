use clap::{arg, Parser, Subcommand};
use hellomouse_board_server::shared::util::config;
use hellomouse_board_server::shared::handlers::postgres_handler::PostgresHandler as SharedPostgresHandler;

#[derive(Parser)]
struct Cli {
    #[clap(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Add a new user
    Add {
        id: String,
        name: String,
        #[arg(value_parser = validate_password)]
        password: String,
    },
    /// Delete a user
    Delete { id: String },
    /// Reset a user's password
    Password {
        id: String,
        #[arg(value_parser = validate_password)]
        password: String,
    },
}

fn validate_password(password: &str) -> Result<String, String> {
    if password.len() < config::get_config().count.min_password_length ||
       password.len() > config::get_config().count.max_password_length {
        return Err(format!("Password must be {} - {} characters (inclusive) in length", 
            config::get_config().count.min_password_length,
            config::get_config().count.max_password_length
        ).to_string());
    }
    Ok(password.to_string())
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    let postgres_handler = SharedPostgresHandler::new().await.unwrap();
    match &cli.command {
        Commands::Add { name, id, password } => match postgres_handler.get_user(id).await {
            Ok(_) => println!(
                "{}",
                format!("Error: User with id `{id}` and name `{name}` already exists")
            ),
            Err(_) => match postgres_handler.create_account(id, name, password).await {
                Ok(_) => println!("Successfully created account"),
                Err(_) => println!("db error"),
            },
        },
        Commands::Delete { id } => match postgres_handler.delete_account(id).await {
            Ok(_) => println!("Successfully deleted account"),
            Err(_) => println!("db error"),
        },
        Commands::Password { id, password } => match postgres_handler.get_user(id).await {
            Ok(_) => match postgres_handler
                .change_password(id, &password)
                .await
            {
                Ok(_) => println!("Successfully changed password"),
                Err(_) => println!("db error"),
            },

            Err(_) => println!("db error"),
        },
    }
}
