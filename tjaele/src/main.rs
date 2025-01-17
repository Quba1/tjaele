//! Heavily inspied by `simple-async` form https://github.com/ratatui/templates

mod app;
mod tui;

use anyhow::{ensure, Result};
use app::App;
use clap::{command, Parser};
use tui::{Event, Tui};

#[derive(Parser)]
#[command(version, about = "Nvidia Fan Control for Wayland", long_about = "long about")]
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

    let mut app = App::init().await?;
    let terminal = ratatui::try_init()?;
    let mut tui = Tui::new(terminal, cli.refresh_interval);

    while app.running {
        tui.draw(&app)?;

        match tui.events.next().await? {
            Event::Tick => app.tick().await,
            Event::Key(key_event) => app.handle_key_events(key_event).await,
        }
    }

    ratatui::try_restore()?;

    Ok(())
}
