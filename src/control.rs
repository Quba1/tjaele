use std::{ffi::OsStr, path::Path, sync::Arc};

use anyhow::{ensure, Context, Result};

use actix_web::{
    error::ErrorInternalServerError, get, post, web, App, HttpRequest, HttpResponse, HttpServer,
    Responder,
};
use nvml_wrapper::Nvml;
use tokio::sync::Mutex;

use crate::{GpuManager, BIND_IP};

#[get("/gpustate")]
async fn gpu_state(req: HttpRequest) -> impl Responder {
    let gpu_manager = req.app_data::<Arc<GpuManager>>().expect("Failed to extract GpuManager");
    let gpu_device_state = gpu_manager.read_state().await;

    match gpu_device_state {
        Ok(state) => HttpResponse::Ok().json(state),
        Err(err) => {
            let mut error_text = format!("Error chain:\n");
            for (i, e) in err.chain().enumerate() {
                error_text.push_str(&format!("[{i}]: {}\n", e.to_string()));
            }
            ErrorInternalServerError(error_text).error_response()
        },
    }
}

pub async fn control_main<P: AsRef<Path>>(config_path: Option<P>) -> Result<()> {
    //this must run as sudo so maybe /etc/config
    let config_path = config_path.unwrap(); // for now unwrap
    let gpu_manager = Arc::new(GpuManager::init(config_path)?);

    HttpServer::new(move || App::new().app_data(gpu_manager.clone()).service(gpu_state))
        .bind((BIND_IP, 8080))?
        .run()
        .await
        .context(
            "Failed to create HTTP server - this is most likely because Tjaele is already running",
        )
}
