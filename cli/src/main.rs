use compiler::{generator::Generator, generators};

#[macro_use]
extern crate tracing;

fn main() {
    setup_logger();

    info!(
        version = %std::env!("CARGO_PKG_VERSION"),
        "Starting website-cli"
    );

    let path = std::env::args().skip(1).next().unwrap();

    let pages = compiler::page::get_pages(&path).unwrap();
    info!(%path, "Got {} pages in path", pages.len());

    for page in pages {
        info!(source = ?page.path, "Generating page");
        let generator = Generator::new(page);
        println!("{}", generator.html().unwrap());
    }
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
