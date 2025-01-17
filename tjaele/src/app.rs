use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use crossterm::event::KeyCode;
use http_body_util::{BodyExt, Empty};
use hyper::{
    body::{Buf, Bytes},
    Request,
};
use hyper_util::rt::TokioIo;
use ratatui::crossterm::{self, event::KeyEvent};
use tjaele_types::{GpuState, SOCKET};
use tokio::net::UnixStream;

#[derive(Debug)]
pub struct App {
    pub latest_data: Result<MonitorData>,
    pub running: bool,
}

#[derive(Debug)]
pub struct MonitorData {
    pub gpu_state: GpuState,
    pub latency: Duration,
}

impl App {
    pub async fn init() -> Result<Self> {
        let latest_data = MonitorData::probe().await;

        Ok(App { running: true, latest_data })
    }

    pub async fn tick(&mut self) {
        self.latest_data = MonitorData::probe().await;
    }

    pub async fn handle_key_events(&mut self, key_event: KeyEvent) {
        match key_event.code {
            KeyCode::Esc | KeyCode::Char('q') => {
                self.running = false;
            },
            _ => {},
        }
    }
}
impl MonitorData {
    pub async fn probe() -> Result<Self> {
        let now = Instant::now();

        let gpu_device_state = UdsClient::fetch_gpu_data()
            .await
            .context("Failed to get tjaele data, is control unit running?")?;

        let elapsed = now.elapsed();

        Ok(MonitorData { gpu_state: gpu_device_state, latency: elapsed })
    }
}

#[derive(Debug)]
pub struct UdsClient;

impl UdsClient {
    /// From Hyper client example
    async fn fetch_gpu_data() -> Result<GpuState> {
        let stream = UnixStream::connect(SOCKET).await?;
        let io = TokioIo::new(stream);

        let (mut sender, conn) = hyper::client::conn::http1::handshake(io).await?;

        // Spawn a task to poll the connection, driving the HTTP state
        tokio::task::spawn(async move {
            // When this error occurs, the function errors
            if (conn.await).is_err() {}
        });

        let req = Request::builder().uri("/gpustate").body(Empty::<Bytes>::new())?;
        let res = sender.send_request(req).await?;
        let body = res.collect().await?.aggregate();

        // try to parse as json with serde_json
        let gpu_state = serde_json::from_reader(body.reader())?;

        Ok(gpu_state)
    }
}
