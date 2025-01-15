use std::{path::Path, sync::Arc};

use actix_web::{
    dev::ServerHandle, error::ErrorInternalServerError, get, App, HttpRequest, HttpResponse,
    HttpServer, Responder,
};
use anyhow::{Context, Result};
use tokio::task;

use crate::{GpuManager, BIND_IP};

pub async fn control_main<P: AsRef<Path>>(config_path: Option<P>) -> Result<()> {
    //this must run as sudo so maybe /etc/config
    let config_path = config_path.unwrap(); // for now unwrap
    let gpu_manager = Arc::new(GpuManager::init(config_path)?);

    let gpu_manager1 = gpu_manager.clone();
    let srv = HttpServer::new(move || App::new().app_data(gpu_manager1.clone()).service(gpu_state))
        .bind((BIND_IP, 8080))?
        .run();

    task::spawn(fan_control(gpu_manager.clone(), srv.handle()));

    srv.await.context(
        "Failed to create HTTP server - this is most likely because Tjaele is already running",
    )
}

#[get("/gpustate")]
async fn gpu_state(req: HttpRequest) -> impl Responder {
    let gpu_manager = req.app_data::<Arc<GpuManager>>().expect("Failed to extract GpuManager");
    let gpu_device_state = gpu_manager.read_state().await;

    match gpu_device_state {
        Ok(state) => HttpResponse::Ok().json(state),
        Err(err) => {
            let mut error_text = "Error chain:\n".to_string();
            for (i, e) in err.chain().enumerate() {
                error_text.push_str(&format!("[{i}]: {}\n", e));
            }
            ErrorInternalServerError(error_text).error_response()
        },
    }
}

async fn fan_control(gpu_manager: Arc<GpuManager>, server_handle: ServerHandle) {
    let mut gpu_temp = 0;
    loop {
        match gpu_manager.set_duty_with_curve(gpu_temp).await {
            Ok(t) => gpu_temp = t,
            Err(e) => {
                println!("Fan control failed with error: {e}. Shutting down.");
                server_handle.stop(true).await
            },
        }
        gpu_manager.sleep().await;
    }
}
