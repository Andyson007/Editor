//! Bundles together the entire editor.
//! This is in order to avoid multiple different cli commands being required to run the server,
//! connect with a client etc.
#[cfg(feature = "security")]
use std::str::FromStr;

use clap::{Args, Parser, Subcommand, ValueEnum};
#[cfg(feature = "security")]
use sqlx::{sqlite::SqliteConnectOptions, SqlitePool};
use tracing::{info, level_filters::LevelFilter};

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
#[command(propagate_version = true)]
struct Cli {
    #[arg(short, long)]
    verbosity: Option<LevelFilter>,
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum Verbosity {
    Error,
    Warning,
    Info,
    Debug,
    Trace,
}

#[derive(Subcommand, Debug)]
enum Commands {
    #[cfg(feature = "security")]
    Server(ServerArgs),
    #[cfg(not(feature = "security"))]
    Server,
    Client(ClientArgs),
}

#[cfg(feature = "security")]
#[derive(Args, Debug)]
struct ServerArgs {
    /// Add a new user which can access files hosted
    #[arg(long, action = clap::ArgAction::SetTrue)]
    add_user: bool,
}

#[derive(Args, Debug)]
struct ClientArgs {
    name: Option<String>,
}

fn main() -> color_eyre::Result<()> {
    let cli = Cli::parse();
    tracing_subscriber::fmt()
        .with_level(true)
        .with_max_level(cli.verbosity.unwrap_or(LevelFilter::OFF))
        .init();
    info!("{cli:?}");

    #[cfg(feature = "security")]
    let pool = tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .unwrap()
        .block_on(SqlitePool::connect_with(
            SqliteConnectOptions::from_str("sqlite://data.db")
                .unwrap()
                .create_if_missing(true),
        ))
        .unwrap();

    match &cli.command {
        #[cfg(not(feature = "security"))]
        Commands::Server => {
            server::run();
        }
        #[cfg(feature = "security")]
        Commands::Server(ServerArgs { add_user: false }) => {
            server::run(pool);
        }
        #[cfg(feature = "security")]
        Commands::Server(ServerArgs { add_user: true }) => {
            let mut stdout = std::io::stdout();
            print!("Enter username: ");
            let mut stdin = std::io::stdin();
            stdout.flush().unwrap();
            let mut username = String::new();
            stdin.read_line(&mut username);
            print!("Enter password: ");

            use std::io::Write;
            use termion::input::TermRead;

            stdout.flush().unwrap();
            let Some(password) = stdin.read_passwd(&mut stdout).unwrap() else {
                // Entering the password was aborted
                std::process::exit(0x82)
            };
            println!("\n{password:?}");
            tokio::runtime::Builder::new_current_thread()
                .enable_time()
                .build()
                .unwrap()
                .block_on(server::add_user(&pool, &username, &password));
        }
        Commands::Client(_) => client::run()?,
    };
    Ok(())
}
