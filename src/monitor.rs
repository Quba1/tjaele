use std::time::{Duration, Instant};

use anyhow::{ensure, Result};
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::Stylize,
    symbols::border,
    text::{Line, Text},
    widgets::{Block, Paragraph, Widget},
    DefaultTerminal, Frame,
};
// no need for blocking client, because Reqwest runs Tokio anyway
use reqwest::{Client, ClientBuilder};

use crate::{GpuState, BIND_IP};

#[derive(Debug)]
pub struct App {
    http_client: Client,
    latest_data: MonitorData,
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
        let latest_data = MonitorData::probe(&http_client).await?;

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
                        if let KeyCode::Char('q') = key_event.code { self.should_exit = true }
                    },
                    _ => {},
                }
            }

            if last_tick.elapsed() >= tick_rate {
                self.latest_data = MonitorData::probe(&self.http_client).await?;
                last_tick = Instant::now();
            }

            if self.should_exit {
                return Ok(());
            }
        }

        Ok(())
    }

    fn draw(&self, frame: &mut Frame) {
        frame.render_widget(self, frame.area());
    }
}

impl Widget for &App {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let title = Line::from("Tjaele Monitor".bold());
        let instructions = Line::from(vec![" Quit ".into(), "<Q> ".blue().bold()]);
        let block = Block::bordered()
            .title(title.centered())
            .title_bottom(instructions.centered())
            .border_set(border::THICK);

        Paragraph::new(self.latest_data.to_tui_text()).block(block).render(area, buf);
    }
}

impl MonitorData {
    pub async fn probe(http_client: &Client) -> Result<Self> {
        let now = Instant::now();

        let gpu_device_state = http_client
            .get(format!("http://{BIND_IP}:8080/gpustate"))
            .send()
            .await?
            .json::<GpuState>()
            .await?;

        let elapsed = now.elapsed();

        Ok(MonitorData { gpu_state: gpu_device_state, latency: elapsed })
    }

    pub fn to_tui_text(&self) -> Text<'static> {
        let latency = self.latency.as_nanos() as f64 / 1_000_000.0;

        let mut lines = vec![Line::from(vec![
            "Monitor Latency: ".to_string().yellow(),
            format!("{} ms", latency).into(),
        ])];

        lines.append(&mut self.gpu_state.render_tui_lines());

        Text::from(lines)
    }
}
