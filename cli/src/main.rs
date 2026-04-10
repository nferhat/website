use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::bail;
use clap::Parser;
use compiler::Generator;

#[macro_use]
extern crate tracing;

#[derive(clap::Parser)]
struct Cli {
    /// The directory containing the source markdown files.
    #[arg(long, short)]
    content_dir: PathBuf,
    /// The path to the main SASS file
    #[arg(long, short)]
    style_input: Option<PathBuf>,
    /// The output directory
    #[arg(long, short)]
    output_dir: PathBuf,
}

fn main() -> anyhow::Result<()> {
    setup_logger();

    info!(
        version = %std::env!("CARGO_PKG_VERSION"),
        "Starting website-cli"
    );

    let cli = Cli::parse();

    _ = std::fs::remove_dir(&cli.output_dir);
    if let Err(err) = std::fs::create_dir_all(&cli.output_dir) {
        bail!("Failed to create output directory: {err:?}")
    }

    if let Some(style_input) = cli.style_input {
        info!(?style_input, "Generating stylesheet from path");
        if let Err(err) = generate_style(style_input, &cli.output_dir) {
            bail!("Failed to generate style: {err:?}");
        }
    }

    let pages = compiler::page::get_pages(&cli.content_dir).unwrap();
    info!(path = ?cli.content_dir, "Got {} pages in path", pages.len());

    for page in pages {
        let output_path = generate_output_path(&page.path, &cli.output_dir);
        info!(source = ?page.path, output = ?output_path, "Generating page");
        if let Err(err) = fs::create_dir_all(output_path.parent().unwrap()) {
            bail!("Failed to create output directory: {err:?}");
        }

        let generator = Generator::new(page, Some("/style.css"));
        let html = generator.html().unwrap();
        if let Err(err) = std::fs::write(&output_path, html) {
            bail!("Failed to generate page: {err:?}");
        }
    }

    Ok(())
}

fn generate_style(style_input: PathBuf, output_path: impl AsRef<Path>) -> anyhow::Result<()> {
    let output_path = output_path.as_ref();
    let sass_input = fs::read_to_string(&style_input)?;
    let style_content = compiler::compile_to_stylesheet(&sass_input).unwrap();
    let style_path = output_path.join("style.css");
    std::fs::write(&style_path, &style_content)?;
    Ok(())
}

fn generate_output_path(path: impl AsRef<Path>, output_dir: impl AsRef<Path>) -> PathBuf {
    let (path, output_dir) = (path.as_ref(), output_dir.as_ref());
    let path = path.components().skip(2).collect::<PathBuf>();
    output_dir.join(&path).with_extension("html")
}

fn setup_logger() {
    use std::str::FromStr as _;
    let filter = tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        // Allow fatal errors from every crate, compositor can log anything
        tracing_subscriber::EnvFilter::from_str("trace").unwrap()
    });
    tracing_subscriber::fmt()
        .compact()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .without_time()
        .init();
}
