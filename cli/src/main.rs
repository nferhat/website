use std::{env, path::PathBuf};

use anyhow::{Context, bail};
use clap::Parser;
use tokio::runtime;

#[macro_use]
extern crate tracing;

mod compiler;
mod config;
mod server;

#[derive(clap::Parser)]
struct Cli {
    /// The configuration path of the website. This determines the root of everything.
    #[arg(long, short)]
    config: Option<PathBuf>,
    /// The command to run.
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Clone, clap::Subcommand)]
pub enum Command {
    /// Builds the website.
    Build,
    /// Serves the website,
    Serve {
        #[arg(long)]
        port: Option<u16>,
    },
}

fn main() -> anyhow::Result<()> {
    setup_logger();

    let cli = Cli::parse();
    // Determine where the root of the website is.
    let root: PathBuf = if let Some(config_path) = &cli.config {
        config_path.parent().context("no parent")?.to_owned()
    } else {
        // Try to use pwd if it contains a website.toml
        let cwd = env::current_dir().context("failed to get cwd")?;
        if cwd.join("website.toml").exists() {
            cwd.to_owned()
        } else {
            bail!("Unable to find website root, make sure there's a website.toml file!");
        }
    };

    let build_dir = root.join("dist");
    std::fs::remove_dir_all(&build_dir)?;
    if let Err(err) = std::fs::create_dir_all(&build_dir) {
        bail!("Failed to create output directory: {err:?}")
    }

    info!(?root, ?build_dir, "Found website");

    let config = config::load(root.join("website.toml"))?;

    let rt = runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;

    rt.block_on(async move {
        match cli.command {
            Command::Build => compiler::build_all(&root, &build_dir, &config, false).await,
            Command::Serve { port } => {
                compiler::build_all(&root, &build_dir, &config, true).await?;
                let port = port.unwrap_or(7272);
                server::run(root, build_dir, config, port).await
            }
        }
    })?;

    Ok(())
}

fn setup_logger() {
    use std::str::FromStr as _;
    let filter = tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        // Allow fatal errors from every crate, compositor can log anything
        tracing_subscriber::EnvFilter::from_str("cli,compiler=debug,info").unwrap()
    });
    tracing_subscriber::fmt()
        .compact()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .without_time()
        .init();

    info!(
        version = %std::env!("CARGO_PKG_VERSION"),
        "Starting website-cli"
    );
}
