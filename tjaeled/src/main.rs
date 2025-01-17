mod gpu_manager;

use std::fmt::Debug;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use std::{path::PathBuf, sync::Arc};

use anyhow::{anyhow, bail, Context, Result};
use clap::{command, Parser};
use gpu_manager::GpuManager;
use http_body_util::Full;
use hyper::body::{Bytes, Incoming};
use hyper::{server::conn::http1, service::service_fn};
use hyper::{Method, Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use tjaele_types::SOCKET;
use tokio::signal::unix::SignalKind;
use tokio::{
    net::{UnixListener, UnixStream},
    select, task,
};
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn, Level};
use tracing_log::LogTracer;

#[derive(Parser)]
#[command(
    version,
    about = "Nvidia Fan Control for Wayland (service)",
    long_about = "long about",
    arg_required_else_help = true
)]
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
        .compact();

    #[cfg(debug_assertions)]
    let subscriber = subscriber.with_max_level(Level::DEBUG);
    #[cfg(not(debug_assertions))]
    let subscriber = subscriber.with_max_level(Level::INFO);

    tracing::subscriber::set_global_default(subscriber.finish())?;
    LogTracer::init()?;

    let cli = Cli::parse();

    let socket_listener = VolatileSocket::bind(SOCKET).context(
        "Failed to bind to socket, this is most likely because another tjaele instance is running or you are running without sudo",
    )?;

    let gpu_manager = task::spawn_blocking(|| GpuManager::init(cli.config_path)).await??;
    let gpu_manager = Arc::new(gpu_manager);
    info!("Successfully initialized connection with NVML");

    let server_token = CancellationToken::new();
    let child_token = server_token.child_token();

    let gpu_manager_clone = gpu_manager.clone();
    tokio::spawn(fan_control(gpu_manager_clone, server_token));

    select! {
        res = unix_socket_server(gpu_manager, socket_listener) => {return res}
        _ = child_token.cancelled() => {error!("Server has been stopped by error in Fan Controller"); bail!("")}
        r = capture_signals() => {return r}
    }
}

#[tracing::instrument]
async fn unix_socket_server(
    gpu_manager: Arc<GpuManager>,
    socket_listener: VolatileSocket,
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

#[tracing::instrument]
async fn handle_socket_stream(io_stream: UnixStream, gpu_manager: Arc<GpuManager>) {
    let io = TokioIo::new(io_stream);
    let gmanager = gpu_manager.clone();

    task::spawn(async move {
        if let Err(err) = http1::Builder::new()
            .serve_connection(io, service_fn(|req| handle_http_request(req, gmanager.clone())))
            .await
        {
            error!("Error serving connection: {err}")
        }
    });
}

#[tracing::instrument]
async fn handle_http_request(
    req: Request<Incoming>,
    gpu_manager: Arc<GpuManager>,
) -> Result<Response<Full<Bytes>>, hyper::http::Error> {
    if req.method() != Method::GET || req.uri().path() != "/gpustate" {
        return Response::builder().status(StatusCode::NOT_FOUND).body(Full::new(Bytes::from("")));
    }

    let gpu_state = task::spawn_blocking(move || gpu_manager.read_state())
        .await
        .map_err(|err| anyhow!("Join error: {err}"))
        .and_then(std::convert::identity) //flatten the error
        .and_then(|state| {
            serde_json::to_string(&state).map_err(|err| anyhow!("Serialization failed: {err}"))
        });

    match gpu_state {
        Ok(state) => {
            let body = Bytes::from(state);
            let body = Full::new(body);
            Response::builder().status(StatusCode::OK).body(body)
        },
        Err(err) => {
            let mut error_text = "Error chain:\n".to_string();
            for (i, e) in err.chain().enumerate() {
                error_text.push_str(&format!("[{i}]: {e}\n"));
            }
            let body = Bytes::from(error_text);
            let body = Full::new(body);
            Response::builder().status(StatusCode::INTERNAL_SERVER_ERROR).body(body)
        },
    }
}

#[tracing::instrument]
async fn fan_control(gpu_manager: Arc<GpuManager>, server_token: CancellationToken) {
    info!("Starting Fan Controller");
    let mut gpu_temp = 0;

    loop {
        let gpu_manager_clone = gpu_manager.clone();
        let fan_control_result =
            task::spawn_blocking(move || gpu_manager_clone.set_duty_with_curve(gpu_temp))
                .await
                .map_err(|err| anyhow!("Join error: {err}"))
                .and_then(std::convert::identity); //flatten the error

        match fan_control_result {
            Ok(t) => gpu_temp = t,
            Err(e) => {
                error!("Fan control failed with error: {e}. Shutting down.");
                server_token.cancel();
            },
        }

        gpu_manager.sleep().await;
    }
}

/// This doesn't handle signals entirely as intended
/// But we need to make sure that we set auto fan policy and
/// remove socket as often as possible
#[tracing::instrument]
async fn capture_signals() -> Result<()> {
    let sigint = tokio::spawn(capture_signal(SignalKind::interrupt()));
    let sigquit = tokio::spawn(capture_signal(SignalKind::quit()));
    let sigterm = tokio::spawn(capture_signal(SignalKind::terminate()));
    let sighup = tokio::spawn(capture_signal(SignalKind::hangup()));

    select! {
        r = sigint => {let r = r?; return r}
        r = sigquit => {let r = r?; return r}
        r = sigterm => {let r = r?; return r}
        r = sighup => {let r = r?; return r}
    }
}

#[tracing::instrument]
async fn capture_signal(signal_kind: SignalKind) -> Result<()> {
    let mut sig = tokio::signal::unix::signal(signal_kind)?;
    sig.recv().await.context("No more signals can be received")?;
    error!("Received signal {signal_kind:?}");

    Ok(())
}

#[derive(Debug)]
struct VolatileSocket {
    path: PathBuf,
    listener: UnixListener,
}

impl VolatileSocket {
    #[tracing::instrument]
    fn bind<T: AsRef<Path> + Debug>(path: T) -> Result<Self> {
        let path = path.as_ref().to_owned();
        let listener = UnixListener::bind(&path).context("Failed to bind socket")?;

        let mut perms = fs::metadata(&path)?.permissions();
        perms.set_mode(0o666);
        fs::set_permissions(&path, perms)?;
        debug!("Socket permissions set");

        Ok(VolatileSocket { path, listener })
    }
}

impl Drop for VolatileSocket {
    fn drop(&mut self) {
        // There's no way to return a useful error here
        std::fs::remove_file(&self.path)
            .expect("Failed to remove tjaeled.sock, please remove it manually");
        debug!("Socket successfully removed");
    }
}

impl std::ops::Deref for VolatileSocket {
    type Target = UnixListener;

    fn deref(&self) -> &Self::Target {
        &self.listener
    }
}

impl std::ops::DerefMut for VolatileSocket {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.listener
    }
}
