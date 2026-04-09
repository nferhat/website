#[macro_use]
extern crate tracing;

fn main() {
    setup_logger();

    info!(
        version = std::env!("CARGO_PKG_VERSION"),
        "Starting website-cli"
    );
}

fn setup_logger() {
    use std::str::FromStr as _;
    let filter = tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        // Allow fatal errors from every crate, compositor can log anything
        tracing_subscriber::EnvFilter::from_str("debug").unwrap()
    });
    tracing_subscriber::fmt()
        .compact()
        .with_env_filter(filter)
        .without_time()
        .init();
}
