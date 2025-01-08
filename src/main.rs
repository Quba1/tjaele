mod control;
mod intermediate_bindings;
mod monitor;

use std::ffi::OsStr;

use control::control_main;
use monitor::monitor_main;

use anyhow::{ensure, Result};
use clap::{Parser, Subcommand};
use nvml_wrapper::Nvml;

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
    Control,
    /// Install the utility to start in the background at startup
    Register,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // recommended path for loading nvml
    let nvml = Nvml::builder().lib_path(OsStr::new("libnvidia-ml.so.1")).init()?;
    ensure!(
        nvml.device_count()? == 1,
        "nvmlcontrol currently supports platforms with one GPU only"
    );

    if let Some(cmd) = cli.command {
        match cmd {
            Commands::Monitor { refresh_interval } => monitor_main(refresh_interval, nvml)?,
            Commands::Control => control_main(&nvml)?,
            Commands::Register => register_main(),
        }
    }

    Ok(())
}

fn register_main() {}
