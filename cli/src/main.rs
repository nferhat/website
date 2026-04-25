use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use clap::Parser;
use compiler::Compiler;
use eyre::{Context as _, ContextCompat as _};
use tokio::runtime;

#[macro_use]
extern crate tracing;

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
    /// Builds all tree-sitter grammars.
    BuildGrammars {
        /// Maximum number of parallel jobs (defaults to number of physical CPUs)
        #[arg(short = 'j', long)]
        jobs: Option<usize>,
    },
}

fn main() -> eyre::Result<()> {
    setup_logger();
    color_eyre::install().unwrap();

    let cli = Cli::parse();
    // Determine where the root of the website is.
    let root: PathBuf = if let Some(config_path) = &cli.config {
        config_path.parent().context("no parent")?.to_owned()
    } else {
        // Try to use pwd if it contains a website.toml
        if PathBuf::from("./website.toml").exists() {
            PathBuf::from("./")
        } else {
            eyre::bail!("Unable to find website root, make sure there's a website.toml file!");
        }
    };

    let build_dir = root.join("dist");
    _ = std::fs::remove_dir_all(&build_dir);
    if let Err(err) = std::fs::create_dir_all(&build_dir) {
        eyre::bail!("Failed to create output directory: {err:?}")
    }

    let config = compiler::Config::load(root.join("website.toml"))?;
    info!(?root, ?build_dir, "Found website");

    let rt = runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;

    rt.block_on(async move {
        match cli.command {
            Command::Build => {
                let mut compiler = Compiler::new(root.clone().into_boxed_path(), config)
                    .await
                    .context("failed to init compiler")?;
                compiler.compile_all().await?;
                Ok(())
            }
            Command::Serve { port } => {
                let port = port.unwrap_or(7272);
                let mut config = config;
                config.base_url = format!("http://localhost:{port}");
                server::run(root, config, port).await
            }
            Command::BuildGrammars { jobs } => {
                let languages_dir = root.join("languages");
                let max_jobs = jobs.unwrap_or_else(|| {
                    std::thread::available_parallelism()
                        .map(|n| n.get())
                        .unwrap_or(1)
                });
                build_all_grammars(&languages_dir, max_jobs).await
            }
        }
    })?;

    Ok(())
}

async fn build_all_grammars(languages_dir: &Path, max_jobs: usize) -> eyre::Result<()> {
    if !languages_dir.exists() {
        eyre::bail!("languages directory does not exist: {:?}", languages_dir);
    }

    // Collect all grammar directories (excluding 'build/')
    let mut grammar_dirs = Vec::new();
    let mut entries = tokio::fs::read_dir(languages_dir)
        .await
        .context("failed to read languages directory")?;

    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        let file_name = entry.file_name();
        let file_name_str = file_name.to_string_lossy();

        if file_name_str == "_build" || !path.is_dir() {
            continue;
        }

        let config_path = path.join("config.toml");
        if config_path.is_file() {
            grammar_dirs.push(file_name_str.to_string());
        }
    }

    if grammar_dirs.is_empty() {
        info!("no grammars found to build");
        return Ok(());
    }

    info!(count = %grammar_dirs.len(), ?max_jobs, "building grammars");

    // Create the grammar cache
    let cache = compiler::highlighter::GrammarCache::new(languages_dir)
        .await
        .context("failed to initialize grammar cache")?;
    let cache = Arc::new(cache);

    let semaphore = Arc::new(tokio::sync::Semaphore::new(max_jobs));

    let jobs: Vec<_> = grammar_dirs
        .into_iter()
        .map(|grammar_name| {
            let cache = Arc::clone(&cache);
            let semaphore = Arc::clone(&semaphore);
            async move {
                let _permit = semaphore.acquire().await.expect("semaphore closed");
                let result = cache.load(&grammar_name).await;
                if let Err(err) = &result {
                    error!(
                        grammar = %grammar_name,
                        error = %err,
                        "failed to build grammar"
                    );
                }
                (grammar_name, result)
            }
        })
        .collect();

    let results = futures::future::join_all(jobs).await;

    // Process results and generate report
    let mut built = Vec::new();
    let mut failed = 0usize;

    for (grammar_id, result) in results {
        match result {
            Ok(_) => {
                built.push(grammar_id);
            }
            Err(_err) => {
                failed += 1;
            }
        }
    }

    // Print the report
    if !built.is_empty() {
        info!(
            count = built.len(),
            grammars = ?built,
            "successfully built grammars"
        );
    }

    if failed > 0 {
        info!("built {} grammars", built.len());
        warn!("failed to build {} grammars", failed);
    } else {
        info!(count = built.len(), "all grammars built successfully");
    }

    Ok(())
}

fn setup_logger() {
    use std::str::FromStr as _;
    let filter = tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        // Allow fatal errors from every crate, compositor can log anything
        tracing_subscriber::EnvFilter::from_str("cli=info,compiler=info,warn").unwrap()
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
