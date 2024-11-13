use clap::{Args, Parser, Subcommand, ValueEnum};
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
    Server,
    Client(ClientArgs),
}

#[derive(Args, Debug)]
struct ClientArgs {
    name: Option<String>,
}

fn main() -> color_eyre::Result<()> {
    let cli = Cli::parse();
    tracing_subscriber::fmt()
        .pretty()
        .with_level(true)
        .with_max_level(dbg!(cli.verbosity.unwrap_or(LevelFilter::OFF)))
        .init();
    info!("{cli:?}");

    // You can check for the existence of subcommands, and if found use their
    // matches just as you would the top level cmd
    match &cli.command {
        Commands::Server => server::run(),
        Commands::Client(_) => client::run()?,
    };
    Ok(())
}
