//! Bundles together the entire editor.
//! This is in order to avoid multiple different cli commands being required to run the server,
//! connect with a client etc.
use clap::{Args, Parser, Subcommand, ValueEnum};
use crossterm::style::Color;
#[cfg(feature = "security")]
use sqlx::{sqlite::SqliteConnectOptions, SqlitePool};
#[cfg(feature = "security")]
use std::str::FromStr;
use std::{
    io::{self, Write},
    net::{Ipv4Addr, SocketAddrV4},
    num::NonZeroU64,
    path::PathBuf,
};
#[cfg(feature = "security")]
use termion::input::TermRead;
use tracing::{level_filters::LevelFilter, trace};

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
#[command(propagate_version = true)]
struct Cli {
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
    /// hosts a server
    Server(ServerArgs),
    /// runs a client which can attach to a server
    Client(ClientArgs),
}

#[derive(Args, Debug)]
struct ServerArgs {
    #[arg(short, long)]
    verbosity: Option<LevelFilter>,
    /// path to the file that should be opened
    ///
    /// it is not a feature yet to share folders
    path: Option<PathBuf>,
    /// disables periodic saves. This forces clients to manually save with `:w`
    #[arg(long, default_value = "false")]
    disable_auto_save: bool,

    /// specifies the time between writes to the save file in seconds
    #[arg(long, default_value = "10")]
    save_interval: NonZeroU64,

    /// IP-address the server should be hosted on
    ///
    /// 0.0.0.0 in order to host on the local network
    #[arg(
        short = 'i',
        long,
        default_value = "127.0.0.1",
        conflicts_with = "address"
    )]
    ip: Ipv4Addr,
    /// Sets the port to listen on
    #[arg(short = 'p', long, default_value = "3012", conflicts_with = "address")]
    port: u16,
    /// Sets the address to host on. This has to be exclive from both ip and port (e.g. 0.0.0.0:5000)
    #[arg(short = 'a', long)]
    address: Option<SocketAddrV4>,
    #[cfg(feature = "security")]
    /// Add a new user which can access files hosted
    #[arg(long, action = clap::ArgAction::SetTrue)]
    add_user: bool,
}

#[derive(Args, Debug)]
struct ClientArgs {
    #[arg(default_value = ".")]
    path: PathBuf,
    #[arg(long, short = 'c', default_value = "green", value_parser = parse_color)]
    color: Color,
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
    #[cfg(feature = "security")]
    #[arg(long)]
    #[allow(clippy::option_option)]
    password: Option<Option<String>>,
    /// IP-address that the server is running on
    ///
    /// By default it checks locally, but for remote access use the ip of that computers ip
    #[arg(
        short = 'i',
        long,
        default_value = "127.0.0.1",
        conflicts_with = "address"
    )]
    ip: Ipv4Addr,
    #[arg(
        short = 'p',
        long = "port",
        default_value = "3012",
        conflicts_with = "address"
    )]
    port: u16,
    /// Sets the address to host on. This has to be exclive from both ip and port (e.g. 10.0.0.10:5000)
    #[arg(short = 'a')]
    address: Option<SocketAddrV4>,
}

fn parse_color(s: &str) -> Result<Color, String> {
    Color::try_from(s.to_lowercase().replace(" ", "_").as_str())
        .map_err(|()| format!("{s} is an invalid color"))
}

fn main() -> color_eyre::Result<()> {
    let cli = Cli::parse();

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
        Commands::Server(ServerArgs {
            path,
            ip,
            port,
            address,
            verbosity,
            disable_auto_save,
            save_interval,
            #[cfg(feature = "security")]
                add_user: false,
        }) => {
            tracing_subscriber::fmt()
                .with_level(true)
                .with_max_level(verbosity.unwrap_or(LevelFilter::OFF))
                .init();
            trace!("{cli:?}");
            let address = address.unwrap_or(SocketAddrV4::new(*ip, *port));
            server::run(
                (!disable_auto_save).then_some(*save_interval),
                address,
                path.as_ref().expect("A path is required to run the server"),
                #[cfg(feature = "security")]
                pool,
            );
        }
        #[cfg(feature = "security")]
        Commands::Server(ServerArgs { add_user: true, .. }) => {
            if let Err(e) = add_user(&pool) {
                match e {
                    sqlx::Error::Database(db) if db.is_unique_violation() => {
                        println!("A user with that username already exists")
                    }
                    _ => println!("An unknown error occurred: {e:?}"),
                }
            }
        }
        Commands::Client(ClientArgs {
            username,
            #[cfg(feature = "security")]
            password,
            ip,
            port,
            address,
            path,
            color,
        }) => {
            let username = username.clone().unwrap_or_else(|| {
                print!("Enter username: ");
                io::stdout().flush().unwrap();
                let mut buf = String::new();
                io::stdin().read_line(&mut buf).unwrap();
                buf.lines().next().unwrap().to_string()
            });
            #[cfg(feature = "security")]
            let password = password.clone().flatten().unwrap_or_else(|| {
                print!("Enter password: ");

                io::stdout().flush().unwrap();
                let Some(password) = io::stdin()
                    .read_passwd(&mut io::stdout())
                    .expect("Stream prematurely ended")
                else {
                    std::process::exit(0x82);
                };
                password
            });
            let address = address.unwrap_or(SocketAddrV4::new(*ip, *port));
            println!("{address}");
            client::run(
                address,
                username.as_str(),
                #[cfg(feature = "security")]
                &password,
                color,
                path,
            )?;
        }
    };
    Ok(())
}

#[cfg(feature = "security")]
fn add_user(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    let mut stdout = std::io::stdout();
    print!("Enter username: ");
    let mut stdin = std::io::stdin();
    stdout.flush().unwrap();
    let mut username = String::new();
    stdin.read_line(&mut username).unwrap();
    print!("Enter password: ");

    use std::io::Write;

    stdout.flush().unwrap();
    let Some(password) = stdin.read_passwd(&mut stdout).unwrap() else {
        // Entering the password was aborted
        std::process::exit(0x82)
    };
    let ret = tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .unwrap()
        .block_on(server::add_user(
            pool,
            username.lines().next().unwrap(),
            password.lines().next().unwrap_or(""),
        ));
    println!();
    ret
}
