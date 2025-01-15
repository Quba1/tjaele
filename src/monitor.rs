#![allow(clippy::cast_precision_loss)]

mod tui_blocks;

use std::time::{Duration, Instant};

use anyhow::{ensure, Context, Result};
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::{
    layout::{Constraint, Direction, Layout},
    DefaultTerminal, Frame,
};
// no need for blocking client, because Reqwest runs Tokio anyway
use reqwest::{Client, ClientBuilder};

use crate::{GpuState, BIND_IP};
use tui_blocks::{
    render_cooling_chart, render_fans_table, DeviceBlock, DriverBlock, ErrorBlock, SpecsBlock,
    TemperatureBlock, TimeBlock,
};

#[derive(Debug)]
pub struct App {
    http_client: Client,
    latest_data: Result<MonitorData>,
    should_exit: bool,
}

#[derive(Debug)]
pub struct MonitorData {
    gpu_state: GpuState,
    latency: Duration,
}

pub async fn monitor_main(refresh_interval: f64) -> Result<()> {
    ensure!(
        refresh_interval > 0.1 && refresh_interval <= 10.0,
        "Monitor refresh interval must be between 0.1 and 10 secods"
    );

    // App is fallibly inited first to not mess up terminal
    let mut app = App::init().await?;
    let mut terminal = ratatui::try_init()?;

    let app_result = app.run(&mut terminal, Duration::from_secs_f64(refresh_interval)).await;
    ratatui::try_restore()?;

    app_result
}

impl App {
    pub async fn init() -> Result<Self> {
        let http_client = ClientBuilder::new().http1_only().build()?;
        let latest_data = MonitorData::probe(&http_client).await;

        Ok(App { should_exit: false, http_client, latest_data })
    }

    pub async fn run(&mut self, terminal: &mut DefaultTerminal, tick_rate: Duration) -> Result<()> {
        let mut last_tick = Instant::now();

        while !self.should_exit {
            terminal.draw(|frame| self.draw(frame))?;

            let timeout = tick_rate.saturating_sub(last_tick.elapsed());
            if event::poll(timeout)? {
                match event::read()? {
                    Event::Key(key_event) if key_event.kind == KeyEventKind::Press => {
                        if let KeyCode::Char('q') = key_event.code {
                            self.should_exit = true;
                        }
                    },
                    _ => {},
                }
            }

            if last_tick.elapsed() >= tick_rate {
                self.latest_data = MonitorData::probe(&self.http_client).await;
                last_tick = Instant::now();
            }

            if self.should_exit {
                return Ok(());
            }
        }

        Ok(())
    }

    fn draw(&self, frame: &mut Frame) {
        match &self.latest_data {
            Ok(data) => App::draw_normal_frame(frame, data),
            Err(err) => App::draw_error_frame(frame, err),
        }
    }

    fn draw_normal_frame(frame: &mut Frame, data: &MonitorData) {
        let main_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints(vec![Constraint::Length(10), Constraint::Fill(1)])
            .split(frame.area());

        let upper_layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(vec![
                Constraint::Ratio(1, 3),
                Constraint::Ratio(1, 3),
                Constraint::Ratio(1, 3),
            ])
            .split(main_layout[0]);

        let lower_layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(vec![Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(main_layout[1]);

        let cooler_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints(vec![
                Constraint::Length(4),
                Constraint::Length(data.gpu_state.persistent.num_fans as u16 + 3),
                Constraint::Fill(1),
            ])
            .split(lower_layout[0]);

        frame.render_widget(TimeBlock { data }, upper_layout[0]);
        frame.render_widget(DeviceBlock { data }, upper_layout[1]);
        frame.render_widget(DriverBlock { data }, upper_layout[2]);
        frame.render_widget(TemperatureBlock { data }, cooler_layout[0]);
        render_fans_table(frame, data, cooler_layout[1]);
        render_cooling_chart(frame, data, cooler_layout[2]);
        frame.render_widget(SpecsBlock { data }, lower_layout[1]);
    }

    fn draw_error_frame(frame: &mut Frame, error: &anyhow::Error) {
        frame.render_widget(ErrorBlock { error }, frame.area());
    }
}

impl MonitorData {
    pub async fn probe(http_client: &Client) -> Result<Self> {
        let now = Instant::now();

        let gpu_device_state = http_client
            .get(format!("http://{BIND_IP}:8080/gpustate"))
            .send()
            .await
            .context("Failed to get tjaele data, is control unit running?")?
            .json::<GpuState>()
            .await
            .context("Tjaele monitor data is malformed")?;

        let elapsed = now.elapsed();

        Ok(MonitorData { gpu_state: gpu_device_state, latency: elapsed })
    }
}
