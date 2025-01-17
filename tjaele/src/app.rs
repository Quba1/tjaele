use std::time::{Duration, Instant};

mod events;
mod tui_blocks;

use anyhow::{Context, Result};
use futures::{FutureExt, StreamExt};
use http_body_util::{BodyExt, Empty};
use hyper::{
    body::{Buf, Bytes},
    Request,
};
use hyper_util::rt::TokioIo;
use ratatui::crossterm::{
    self,
    event::{Event as CrosstermEvent, KeyEvent},
};
use tjaele_types::{GpuState, SOCKET};
use tokio::{net::UnixStream, sync::mpsc};

#[derive(Debug)]
pub struct App {
    socket_client: UdsClient,
    latest_data: Result<MonitorData>,
    is_running: bool,
}

#[derive(Debug)]
pub struct MonitorData {
    gpu_state: GpuState,
    latency: Duration,
}

impl App {
    pub async fn init() -> Result<Self> {
        let socket_client = UdsClient;
        let latest_data = MonitorData::probe(&socket_client).await;

        Ok(App { is_running: true, latest_data, socket_client })
    }
}

impl MonitorData {
    pub async fn probe(client: &UdsClient) -> Result<Self> {
        let now = Instant::now();

        let gpu_device_state = client
            .fetch_gpu_data()
            .await
            .context("Failed to get tjaele data, is control unit running?")?;

        let elapsed = now.elapsed();

        Ok(MonitorData { gpu_state: gpu_device_state, latency: elapsed })
    }
}

#[derive(Debug)]
struct UdsClient;

impl UdsClient {
    /// From Hyper client example
    async fn fetch_gpu_data(&self) -> Result<GpuState> {
        let stream = UnixStream::connect(SOCKET).await?;
        let io = TokioIo::new(stream);

        let (mut sender, conn) = hyper::client::conn::http1::handshake(io).await?;

        // Spawn a task to poll the connection, driving the HTTP state
        tokio::task::spawn(async move {
            // When this error occurs, the function errors
            if let Err(_) = conn.await {}
        });

        let req = Request::builder().uri("/gpustate").body(Empty::<Bytes>::new())?;
        let res = sender.send_request(req).await?;
        let body = res.collect().await?.aggregate();

        // try to parse as json with serde_json
        let gpu_state = serde_json::from_reader(body.reader())?;

        Ok(gpu_state)
    }
}
