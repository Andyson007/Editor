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
    net::{Ipv4Addr, SocketAddrV4},
    path::PathBuf,
};
use termion::input::TermRead;
use tracing::{info, level_filters::LevelFilter};

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
    path: PathBuf,
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
    /// Sets the address to host on. This has to be exclive from both ip and port
    #[arg(short = 'a', long)]
    address: Option<SocketAddrV4>,
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
    #[allow(clippy::option_option)]
    password: Option<Option<String>>,
    /// IP-address the server should be hosted on
    ///
    /// 0.0.0.0 in order to host on the local network
    #[arg(short = 'i', default_value = "127.0.0.1", conflicts_with = "address")]
    ip: Ipv4Addr,
    #[arg(default_value = "3012", conflicts_with = "address")]
    port: u16,
    #[arg(short = 'a')]
    address: Option<SocketAddrV4>,
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
            #[cfg(feature = "security")]
                add_user: false,
        }) => {
            tracing_subscriber::fmt()
                .with_level(true)
                .with_max_level(verbosity.unwrap_or(LevelFilter::OFF))
                .init();
            info!("{cli:?}");
            let address = address.unwrap_or(SocketAddrV4::new(*ip, *port));
            server::run(
                address,
                path,
                #[cfg(feature = "security")]
                pool,
            );
        }
        #[cfg(feature = "security")]
        Commands::Server(ServerArgs { add_user: true, .. }) => {
            add_user(&pool);
        }
        Commands::Client(ClientArgs {
            username,
            password,
            ip,
            port,
            address,
        }) => {
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
            let address = address.unwrap_or(SocketAddrV4::new(*ip, *port));
            client::run(address, username.as_str(), password.as_deref())?;
        }
    };
    Ok(())
}

#[cfg(feature = "security")]
fn add_user(pool: &SqlitePool) {
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
