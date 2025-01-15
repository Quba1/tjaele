#![allow(unused)]

mod control;
mod gpu_manager;
mod monitor;

use std::path::PathBuf;

use control::control_main;
use monitor::monitor_main;

use anyhow::Result;
use clap::{Parser, Subcommand};

pub use gpu_manager::{GpuManager, GpuState};

// Probability of IP collision is virtually zero
const BIND_IP: &'static str = "127.45.62.73";

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

#[tokio::main]
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

fn register_main() {}
