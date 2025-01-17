//! Heavily inspied by `simple-async` form https://github.com/ratatui/templates

mod app;

use anyhow::{ensure, Result};
use app::App;
use clap::{command, Parser};

#[derive(Parser)]
#[command(
    version,
    about = "Nvidia Fan Control for Wayland",
    long_about = "long about",
)]
struct Cli {
    /// Monitor refresh interval in seconds
    #[arg(short, long, default_value_t = 2.0)]
    refresh_interval: f64,
}

#[tokio::main(worker_threads = 4)]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    ensure!(
        cli.refresh_interval > 0.1 && cli.refresh_interval <= 10.0,
        "Monitor refresh interval must be between 0.1 and 10 secods"
    );

    let app = App::init().await?;

    dbg!(app);

    Ok(())
}
