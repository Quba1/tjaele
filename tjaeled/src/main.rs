mod gpu_manager;

use std::{path::PathBuf, sync::Arc};

use anyhow::{anyhow, Context, Result};
use clap::{command, Parser};
use gpu_manager::GpuManager;
use tjaele_types::GpuState;
use tokio::{
    net::{UnixListener, UnixStream},
    select, task,
};
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, Level};
use tracing_log::LogTracer;

#[derive(Parser)]
#[command(version, about = "about", long_about = "long about", arg_required_else_help = true)]
struct Cli {
    /// Path to the configuration file
    #[arg(short, long, required = true)]
    config_path: PathBuf,
}

#[tokio::main(worker_threads = 4)]
#[tracing::instrument]
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

    let socket_listener = UnixListener::bind("/var/run/tjaele/tjaele.sock").context(
        "Failed to bind to socket, this is most likely because another tjaele instance is running",
    )?;

    let gpu_manager = task::spawn_blocking(|| GpuManager::init(cli.config_path)).await??;
    let gpu_manager = Arc::new(gpu_manager);
    info!("Successfully initialized connection with NVML");

    let server_token = CancellationToken::new();
    let child_token = server_token.child_token();

    let gpu_manager_clone = gpu_manager.clone();
    tokio::spawn(fan_control(gpu_manager_clone, server_token));

    select! {
        _ = child_token.cancelled() => {return Err(anyhow!("Server has been stopped by error in Fan Controller"))}
        res = unix_socket_server(gpu_manager, socket_listener) => {return res}
    }
}

#[tracing::instrument]
async fn unix_socket_server(
    gpu_manager: Arc<GpuManager>,
    socket_listener: UnixListener,
) -> Result<()> {
    loop {
        match socket_listener.accept().await {
            Ok((stream, _addr)) => {
                debug!("Received new client on Unix Socket");
                let gmanager = gpu_manager.clone();
                tokio::spawn(handle_socket_stream(stream, gmanager));
            },
            Err(e) => {
                error!("Unix Socket accept() returned error {e}")
            },
        }
    }
}

async fn handle_socket_stream(stream: UnixStream, gpu_manager: Arc<GpuManager>) {
    // errors need to be handled internally because we can't propagate them up

    // connect
    // validate request type
    // respond to request

    let gpu_state = task::spawn_blocking(|| get_gpu_state(gpu_manager)).await;

    todo!()
}

#[tracing::instrument]
fn get_gpu_state(gpu_manager: Arc<GpuManager>) -> Result<GpuState> {
    let gpu_device_state = gpu_manager.read_state();

    // to enum
    // serialize

    // match gpu_device_state {
    //     Ok(state) => HttpResponse::Ok().json(state),
    //     Err(err) => {
    //         let mut error_text = "Error chain:\n".to_string();
    //         for (i, e) in err.chain().enumerate() {
    //             error_text.push_str(&format!("[{i}]: {e}\n"));
    //         }
    //         ErrorInternalServerError(error_text).error_response()
    //     },
    // }
    todo!()
}

#[tracing::instrument]
async fn fan_control(gpu_manager: Arc<GpuManager>, server_token: CancellationToken) {
    info!("Starting Fan Controller");
    let mut gpu_temp = 0;

    loop {
        let gpu_manager_clone = gpu_manager.clone();
        let fan_control_result =
            task::spawn_blocking(move || gpu_manager_clone.set_duty_with_curve(gpu_temp)).await;

        // match fan_control_result {
        //     Ok(Ok(_)) => todo!(),
        //     Err(_) => todo!(),
        // }

        // match gpu_manager.set_duty_with_curve(gpu_temp) {
        //     Ok(t) => gpu_temp = t,
        //     Err(e) => {
        //         error!("Fan control failed with error: {e}. Shutting down.");
        //         todo!("Stop server")
        //     },
        // }

        gpu_manager.sleep().await;
    }
}
