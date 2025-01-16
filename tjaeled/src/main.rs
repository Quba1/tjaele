mod gpu_manager;

use std::{path::PathBuf, sync::Arc};

use anyhow::Result;
use clap::{command, Parser};
use gpu_manager::GpuManager;
use tracing::{info, Level};
use tracing_log::LogTracer;

#[derive(Parser)]
#[command(version, about = "about", long_about = "long about", arg_required_else_help = true)]
struct Cli {
    /// Path to the configuration file
    #[arg(short, long, required = true)]
    config_path: PathBuf,
}

#[tokio::main(worker_threads = 4)]
async fn main() -> Result<()> {
    let subscriber = tracing_subscriber::fmt()
        .with_file(true)
        .with_line_number(true)
        .with_thread_ids(true)
        .with_target(true)
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;
    LogTracer::init()?;

    let cli = Cli::parse();
    // Technically this is blocking, but there are no other tasks at this point
    // Thus we can block the thread for a little bit
    let gpu_manager = Arc::new(GpuManager::init(cli.config_path)?);
    info!("Successfully initialized connection with NVML");

    Ok(())
}
