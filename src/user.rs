use clap::{arg, Args, Parser, Subcommand};
use hellomouse_board_server::shared::handlers::postgres_handler::PostgresHandler as SharedPostgresHandler;
use std::env;

#[derive(Parser)]
struct Cli {
    #[clap(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Add {
        id: String,
        name: String,
        #[arg(value_parser = validate_password)]
        password: String,
    },
    Delete {
        id: String,
    },
}

fn validate_password(password: &str) -> Result<String, String> {
    if password.len() < 10 {
        return Err("Password must be 10 characters or more in length".to_string());
    }
    Ok(password.to_string())
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    let mut postgres_handler = SharedPostgresHandler::new().await.unwrap();
    match &cli.command {
        Commands::Add { name, id, password } => match postgres_handler.get_user(id).await {
            Ok(_) => {
                println!(
                    "{}",
                    format!("Error: User with id `{id}` and name `{name}` already exists")
                );
            }
            Err(_) => match postgres_handler.create_account(id, name, password).await {
                Ok(_) => {
                    println!("Successfully created account")
                }
                Err(_) => {
                    println!("db error")
                }
            },
        },
        Commands::Delete { id } => match postgres_handler.delete_account(id).await {
            Ok(_) => {
                println!("Successfully deleted account")
            }
            Err(_) => {
                println!("db error")
            }
        },
    }
}