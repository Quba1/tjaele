#![warn(clippy::pedantic)]
#![warn(clippy::cargo)]
#![warn(clippy::perf)]
#![allow(clippy::must_use_candidate)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::multiple_crate_versions)]

mod control;
mod gpu_manager;
mod monitor;

use std::path::PathBuf;

use control::control_main;
use monitor::monitor_main;

use anyhow::Result;
use clap::{Parser, Subcommand};

pub use gpu_manager::{GpuManager, GpuState};

#[derive(Parser)]
#[command(version, about = "about", long_about = "long about", arg_required_else_help = true)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Show current GPU stats
    Monitor {
        #[arg(short, long, default_value_t = 2.0)]
        refresh_interval: f64,
    },
    /// Run the GPU control loop
    Control {
        #[arg(short, long)]
        config_path: Option<PathBuf>,
    },
}

#[tokio::main(worker_threads = 4)]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    if let Some(cmd) = cli.command {
        match cmd {
            Commands::Monitor { refresh_interval } => monitor_main(refresh_interval).await?,
            Commands::Control { config_path } => control_main(config_path).await?,
        }
    }

    Ok(())
}
