use std::{
    fmt::Debug,
    path::{Path, PathBuf},
    sync::Arc,
};

use actix_web::{
    dev::ServerHandle, error::ErrorInternalServerError, get, App, HttpRequest, HttpResponse,
    HttpServer, Responder,
};
use anyhow::{Context, Result};
use tokio::task;
use tracing::{error, info, Level};
use tracing_log::LogTracer;

use crate::{GpuManager, BIND_IP};

#[tracing::instrument]
pub async fn control_main<P: AsRef<Path> + Debug>(config_path: Option<P>) -> Result<()> {
    let subscriber = tracing_subscriber::fmt()
        .with_file(true)
        .with_line_number(true)
        .with_thread_ids(true)
        .with_target(true)
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    LogTracer::init()?;

    let config_path = match config_path {
        Some(p) => p.as_ref().to_owned(),
        None => PathBuf::from("/etc/tjaele/config.toml"),
    };
    let gpu_manager = Arc::new(GpuManager::init(config_path)?);

    info!("Successfully initialized connection with NVML");

    let gpu_manager1 = gpu_manager.clone();
    let srv = HttpServer::new(move || App::new().app_data(gpu_manager1.clone()).service(gpu_state))
        .workers(4)
        .bind((BIND_IP, 8080))?
        .run();

    task::spawn(fan_control(gpu_manager.clone(), srv.handle()));

    srv.await.context(
        "Failed to create HTTP server - this is most likely because Tjaele is already running",
    )
}

#[get("/gpustate")]
#[tracing::instrument]
async fn gpu_state(req: HttpRequest) -> impl Responder {
    let gpu_manager = req.app_data::<Arc<GpuManager>>().expect("Failed to extract GpuManager");
    let gpu_device_state = gpu_manager.read_state().await;

    match gpu_device_state {
        Ok(state) => HttpResponse::Ok().json(state),
        Err(err) => {
            let mut error_text = "Error chain:\n".to_string();
            for (i, e) in err.chain().enumerate() {
                error_text.push_str(&format!("[{i}]: {e}\n"));
            }
            ErrorInternalServerError(error_text).error_response()
        },
    }
}

#[tracing::instrument]
async fn fan_control(gpu_manager: Arc<GpuManager>, server_handle: ServerHandle) {
    let mut gpu_temp = 0;
    loop {
        match gpu_manager.set_duty_with_curve(gpu_temp).await {
            Ok(t) => gpu_temp = t,
            Err(e) => {
                error!("Fan control failed with error: {e}. Shutting down.");
                server_handle.stop(true).await;
            },
        }
        gpu_manager.sleep().await;
    }
}
