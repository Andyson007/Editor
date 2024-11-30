//! Bundles together the entire editor.
//! This is in order to avoid multiple different cli commands being required to run the server,
//! connect with a client etc.
use clap::{Args, Parser, Subcommand, ValueEnum};
#[cfg(feature = "security")]
use sqlx::{sqlite::SqliteConnectOptions, SqlitePool};
#[cfg(feature = "security")]
use std::str::FromStr;
use std::{
    io::{self, Write},
    path::PathBuf,
};
use termion::input::TermRead;
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
    Server(ServerArgs),
    Client(ClientArgs),
}

#[derive(Args, Debug)]
struct ServerArgs {
    /// path to the file that should be opened
    ///
    /// it is not a feature yet to share folders
    path: PathBuf,
    #[cfg(feature = "security")]
    /// Add a new user which can access files hosted
    #[arg(long, action = clap::ArgAction::SetTrue)]
    add_user: bool,
}

#[derive(Args, Debug)]
struct ClientArgs {
    #[arg(long, short = 'u')]
    /// Supply the username inline.
    ///
    /// Not supplying a username will prompt for it
    username: Option<String>,
    /// Supply the password
    ///
    /// When not present no password will be assumed. A password might be required however if the
    /// target server is running with security enabled.
    /// A prompt will appear if you don't specify the password with the flag
    #[arg(long, short = 'p')]
    password: Option<Option<String>>,
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
        Commands::Server(ServerArgs { path }) => {
            server::run(path);
        }
        #[cfg(feature = "security")]
        Commands::Server(ServerArgs {
            path,
            add_user: false,
        }) => {
            server::run(path, pool);
        }
        #[cfg(feature = "security")]
        Commands::Server(ServerArgs {
            path: _,
            add_user: true,
        }) => {
            let mut stdout = std::io::stdout();
            print!("Enter username: ");
            let mut stdin = std::io::stdin();
            stdout.flush().unwrap();
            let mut username = String::new();
            stdin.read_line(&mut username);
            print!("Enter password: ");

            use std::io::Write;

            stdout.flush().unwrap();
            let Some(password) = stdin.read_passwd(&mut stdout).unwrap() else {
                // Entering the password was aborted
                std::process::exit(0x82)
            };
            tokio::runtime::Builder::new_current_thread()
                .enable_time()
                .build()
                .unwrap()
                .block_on(server::add_user(
                    &pool,
                    &username.lines().next().unwrap(),
                    &password.lines().next().unwrap(),
                ));
        }
        Commands::Client(ClientArgs { username, password }) => {
            let username = username.clone().unwrap_or_else(|| {
                print!("Enter username: ");
                io::stdout().flush().unwrap();
                let mut buf = String::new();
                io::stdin().read_line(&mut buf).unwrap();
                buf.lines().next().unwrap().to_string()
            });
            let password = password.as_ref().map(|x| {
                x.clone().unwrap_or_else(|| {
                    print!("Enter password: ");

                    io::stdout().flush().unwrap();
                    let Some(password) = io::stdin().read_passwd(&mut io::stdout()).unwrap() else {
                        std::process::exit(0x82);
                    };
                    password
                })
            });
            client::run(username.as_str(), password.as_deref())?
        }
    };
    Ok(())
}
