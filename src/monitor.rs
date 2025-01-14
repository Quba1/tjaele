use std::time::{Duration, Instant};

use anyhow::{ensure, Context, Result};
use chrono::{DateTime, Local};
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use nvml_wrapper::{
    cuda_driver_version_major, cuda_driver_version_minor,
    enum_wrappers::device::{Clock, TemperatureSensor, TemperatureThreshold},
    enums::device::DeviceArchitecture,
    struct_wrappers::device::MemoryInfo,
    structs::device::CudaComputeCapability,
    Nvml,
};
use pretty_bytes::converter::convert;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::Stylize,
    symbols::border,
    text::{Line, Text},
    widgets::{Block, Paragraph, Widget},
    DefaultTerminal, Frame,
};

use crate::intermediate_bindings::{AdditionalNvmlFunctionality, MinMaxFanSpeeds};

#[derive(Debug)]
pub struct FanState {
    pub speed: u32,
    pub control_policy: u32,
}

#[derive(Debug)]
pub struct App {
    nvml: Nvml,
    nvml_state: NvmlState,
    should_exit: bool,
}

pub fn monitor_main(refresh_interval: f64, nvml: Nvml) -> Result<()> {
    ensure!(
        refresh_interval > 0.1 && refresh_interval <= 10.0,
        "Monitor refresh interval must be between 0.1 and 10 secods"
    );

    let mut terminal = ratatui::init();
    let app_result = App::init(nvml)?.run(&mut terminal, Duration::from_secs_f64(refresh_interval));
    // TODO: error breaks terminal (although it shouldn't)
    ratatui::restore();
    app_result
}

impl App {
    pub fn init(nvml: Nvml) -> Result<Self> {
        let nvml_state = NvmlState::probe(&nvml)?;

        Ok(App { nvml, nvml_state, should_exit: false })
    }

    pub fn run(&mut self, terminal: &mut DefaultTerminal, tick_rate: Duration) -> Result<()> {
        let mut last_tick = Instant::now();

        while !self.should_exit {
            terminal.draw(|frame| self.draw(frame))?;

            let timeout = tick_rate.saturating_sub(last_tick.elapsed());
            if event::poll(timeout)? {
                match event::read()? {
                    Event::Key(key_event) if key_event.kind == KeyEventKind::Press => {
                        match key_event.code {
                            KeyCode::Char('q') => self.should_exit = true,
                            _ => {},
                        }
                    },
                    _ => {},
                }
            }

            if last_tick.elapsed() >= tick_rate {
                self.nvml_state = NvmlState::probe(&self.nvml)?;
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
        let title = Line::from("Nvidia GPU Monitor".bold());
        let instructions = Line::from(vec![" Quit ".into(), "<Q> ".blue().bold()]);
        let block = Block::bordered()
            .title(title.centered())
            .title_bottom(instructions.centered())
            .border_set(border::THICK);

        let monitor_text = self.nvml_state.text();

        Paragraph::new(monitor_text).block(block).render(area, buf);
    }
}

#[derive(Debug)]
struct NvmlState {
    probe_time: DateTime<Local>,
    sys_driver_version: String,
    cuda_version_major: i32,
    cuda_version_minor: i32,
    cuda_capability: CudaComputeCapability,
    nvml_version: String,
    device_name: String,
    architecture: DeviceArchitecture,
    cuda_cores: u32,
    num_fans: u32,
    link_gen: u32,
    link_width: u32,
    link_speed: u64,
    max_link_gen: u32,
    max_link_width: u32,
    max_link_speed: u64,
    mem_info: MemoryInfo,
    graphics_clock: u32,
    mem_clock: u32,
    power_usage: f64,
    shutdown_temperature: u32,
    slowdown_temperature: u32,
    gpumax_temperature: u32,
    temperature: u32,
    fans_state: Vec<FanState>,
    minmax_fan_speed: MinMaxFanSpeeds,
}

impl NvmlState {
    pub fn probe(nvml: &Nvml) -> Result<Self> {
        let probe_time = chrono::Local::now();

        let device = nvml.device_by_index(0)?;

        let sys_driver_version = nvml.sys_driver_version()?;
        let cuda_version = nvml.sys_cuda_driver_version()?;
        let cuda_version_major = cuda_driver_version_major(cuda_version);
        let cuda_version_minor = cuda_driver_version_minor(cuda_version);
        let cuda_capability = device.cuda_compute_capability()?;
        let nvml_version = nvml.sys_nvml_version()?;

        let device_name = device.name()?;
        let architecture = device.architecture()?;
        let cuda_cores = device.num_cores()?;
        let num_fans = device.num_fans()?;

        let link_gen = device.current_pcie_link_gen()?;
        let link_speed = device.pcie_link_speed().map(u64::from).map(|x| x * 1000000)?;
        let link_width = device.current_pcie_link_width()?;
        let max_link_gen = device.max_pcie_link_gen()?;
        let max_link_width = device.max_pcie_link_width()?;
        let max_link_speed = device
            .max_pcie_link_speed()?
            .as_integer()
            .map(u64::from)
            .map(|x| x * 1000000)
            .context("Couldn't convert PCIe max link speed")?;

        let mem_info = device.memory_info()?;

        let graphics_clock = device.clock_info(Clock::Graphics)?;
        let mem_clock = device.clock_info(Clock::Memory)?;

        let power_usage = f64::from(device.power_usage()?) / 1000.0;

        let shutdown_temperature = device.temperature_threshold(TemperatureThreshold::Shutdown)?;
        let slowdown_temperature = device.temperature_threshold(TemperatureThreshold::Slowdown)?;
        let gpumax_temperature = device.temperature_threshold(TemperatureThreshold::GpuMax)?;

        let temperature = device.temperature(TemperatureSensor::Gpu)?;
        let fans_state = (0..num_fans)
            .map(|fan_idx| -> Result<FanState> {
                Ok(FanState {
                    speed: device.fan_speed(fan_idx).context("Failed to read fan speed")?,
                    control_policy: device
                        .fan_control_policy(fan_idx)
                        .context("Failed to read fan policy")?,
                })
            })
            .collect::<Result<Vec<_>>>()?;

        let minmax_fan_speed = device.min_max_fan_speed()?;

        Ok(Self {
            probe_time,
            sys_driver_version,
            cuda_version_major,
            cuda_version_minor,
            cuda_capability,
            nvml_version,
            device_name,
            architecture,
            cuda_cores,
            num_fans,
            link_gen,
            link_width,
            link_speed,
            max_link_gen,
            max_link_width,
            max_link_speed,
            mem_info,
            graphics_clock,
            mem_clock,
            power_usage,
            shutdown_temperature,
            slowdown_temperature,
            gpumax_temperature,
            temperature,
            fans_state,
            minmax_fan_speed,
        })
    }

    pub fn text(&self) -> Text<'static> {
        let mut lines = Vec::new();
        lines.push(Line::from(vec![
            "System Time: ".to_string().yellow(),
            format!("{}", self.probe_time.to_rfc2822()).into(),
        ]));
        lines.push(Line::from(vec!["".into()]));

        lines.push(Line::from(vec![
            "Device: ".to_string().yellow(),
            format!(
                "{} (architecture: {}, CUDA cores: {}, fans: {})",
                self.device_name, self.architecture, self.cuda_cores, self.num_fans
            )
            .into(),
        ]));
        lines.push(Line::from(vec![
            "Temperature thresholds: ".to_string().yellow(),
            format!(
                "{} C (shutdown), {} C (slowdown), {} C (gpumax)",
                self.shutdown_temperature, self.slowdown_temperature, self.gpumax_temperature
            )
            .into(),
        ]));
        lines.push(Line::from(vec!["".into()]));

        lines.push(Line::from(vec![
            "Driver version: ".to_string().yellow(),
            format!("{}", self.sys_driver_version).into(),
        ]));
        lines.push(Line::from(vec![
            "CUDA Driver version: ".to_string().yellow(),
            format!("{}.{}", self.cuda_version_major, self.cuda_version_minor).into(),
        ]));
        lines.push(Line::from(vec![
            "CUDA Compute Capability: ".to_string().yellow(),
            format!("{}.{}", self.cuda_capability.major, self.cuda_capability.minor).into(),
        ]));
        lines.push(Line::from(vec![
            "NVML version: ".to_string().yellow(),
            format!("{}", self.nvml_version).into(),
        ]));
        lines.push(Line::from(vec!["".into()]));

        lines.push(Line::from(vec![
            "Current PCIe connection: ".to_string().yellow(),
            format!(
                "{}x{} ({})",
                self.link_gen,
                self.link_width,
                convert(self.link_speed as _).replace("B", "T") + "/s",
            )
            .into(),
        ]));
        lines.push(Line::from(vec![
            "Maximum PCIe connection: ".to_string().yellow(),
            format!(
                "{}x{} ({})",
                self.max_link_gen,
                self.max_link_width,
                convert(self.max_link_speed as _).replace("B", "T") + "/s",
            )
            .into(),
        ]));
        lines.push(Line::from(vec!["".into()]));

        lines.push(Line::from(vec![
            "Theoretical applicable fan speeds: ".to_string().yellow(),
            format!("{}% (min), {}% (max)", self.minmax_fan_speed.min, self.minmax_fan_speed.max,)
                .into(),
        ]));
        lines.push(Line::from(vec!["".into()]));

        lines.push(Line::from(vec![
            "Memory: ".to_string().yellow(),
            format!(
                "{} (used), {} (total)",
                convert(self.mem_info.used as _),
                convert(self.mem_info.total as _),
            )
            .into(),
        ]));
        lines.push(Line::from(vec!["".into()]));

        lines.push(Line::from(vec![
            "Clock: ".to_string().yellow(),
            format!("{} MHz (graphics), {} MHz (memory)", self.graphics_clock, self.mem_clock,)
                .into(),
        ]));
        lines.push(Line::from(vec!["".into()]));

        lines.push(Line::from(vec![
            "Power usage: ".to_string().yellow(),
            format!("{:.3} W", self.power_usage,).into(),
        ]));
        lines.push(Line::from(vec!["".into()]));

        lines.push(Line::from(vec![
            "GPU Temperature: ".to_string().yellow(),
            format!("{} C", self.temperature).into(),
        ]));

        let mut fan_line = vec!["Fans speed:".to_string().yellow()];
        self.fans_state.iter().enumerate().for_each(|(idx, state)| {
            fan_line.push(
                format!(" <fan_{} speed: {}% policy: {}>", idx, state.speed, state.control_policy,)
                    .into(),
            )
        });
        lines.push(Line::from(fan_line));
        lines.push(Line::from(vec!["".into()]));

        Text::from(lines)
    }
}
