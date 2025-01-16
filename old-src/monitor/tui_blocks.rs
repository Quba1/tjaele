use pretty_bytes::converter::convert;
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Rect},
    style::{Color, Style, Stylize},
    symbols::{border, Marker},
    text::{Line, Text},
    widgets::{Axis, Block, Chart, Dataset, GraphType, Paragraph, Row, Table, Widget},
    Frame,
};

use super::MonitorData;

pub(super) struct TimeBlock<'a> {
    pub(super) data: &'a MonitorData,
}

pub(super) struct DeviceBlock<'a> {
    pub(super) data: &'a MonitorData,
}

pub(super) struct DriverBlock<'a> {
    pub(super) data: &'a MonitorData,
}

pub(super) struct TemperatureBlock<'a> {
    pub(super) data: &'a MonitorData,
}

pub(super) struct SpecsBlock<'a> {
    pub(super) data: &'a MonitorData,
}

pub(super) struct ErrorBlock<'a> {
    pub(super) error: &'a anyhow::Error,
}

impl Widget for TimeBlock<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let title = Line::from("Tjaele Monitor".bold());
        let block = Block::bordered().title(title.left_aligned()).border_set(border::PLAIN);

        let latency = self.data.latency.as_nanos() as f64 / 1_000_000.0;

        let text = Text::from(vec![
            Line::from("System Time".to_string().yellow()),
            Line::from(self.data.gpu_state.runtime.probe_time.to_rfc2822()),
            Line::from(""),
            Line::from("GPU Probe Latency".to_string().yellow()),
            Line::from(format!("{latency:9.6} ms")),
        ]);

        Paragraph::new(text).block(block).render(area, buf);
    }
}

impl Widget for DeviceBlock<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let title = Line::from("GPU Info".bold());
        let block = Block::bordered().title(title.left_aligned()).border_set(border::PLAIN);

        let text = Text::from(vec![
            Line::from("Device".to_string().yellow()),
            Line::from(format!(
                "{} ({} Architecture)",
                self.data.gpu_state.persistent.device_name,
                self.data.gpu_state.persistent.architecture
            )),
            Line::from(""),
            Line::from("CUDA Cores".to_string().yellow()),
            Line::from(self.data.gpu_state.persistent.num_cores.to_string()),
            Line::from(""),
            Line::from("Fans Count".to_string().yellow()),
            Line::from(self.data.gpu_state.persistent.num_fans.to_string()),
        ]);

        Paragraph::new(text).block(block).render(area, buf);
    }
}

impl Widget for DriverBlock<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let title = Line::from("Driver Info".bold());
        let block = Block::bordered().title(title.left_aligned()).border_set(border::PLAIN);

        let text = Text::from(vec![
            Line::from("Nvidia Driver Version".to_string().yellow()),
            Line::from(self.data.gpu_state.persistent.sys_info.driver_version.to_string()),
            Line::from(""),
            Line::from("CUDA Driver Version / Compute Capability".to_string().yellow()),
            Line::from(format!(
                "{}.{} / {}.{}",
                self.data.gpu_state.persistent.sys_info.cuda_version.major,
                self.data.gpu_state.persistent.sys_info.cuda_version.minor,
                self.data.gpu_state.persistent.sys_info.cuda_capability.major,
                self.data.gpu_state.persistent.sys_info.cuda_capability.minor
            )),
            Line::from(""),
            Line::from("NVML Version".to_string().yellow()),
            Line::from(self.data.gpu_state.persistent.sys_info.nvml_version.to_string()),
        ]);

        Paragraph::new(text).block(block).render(area, buf);
    }
}

impl Widget for TemperatureBlock<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let title = Line::from("Temperatures".bold());
        let block = Block::bordered().title(title.left_aligned()).border_set(border::PLAIN);

        let text = Text::from(vec![
            Line::from("GPU".to_string().yellow()),
            Line::from(format!("{} C", self.data.gpu_state.runtime.device_temperature)),
        ]);

        Paragraph::new(text).block(block).render(area, buf);
    }
}

pub fn render_fans_table(frame: &mut Frame, data: &MonitorData, area: Rect) {
    let title = Line::from("Fans".bold());
    let block = Block::bordered().title(title.left_aligned()).border_set(border::PLAIN);

    let rows = data
        .gpu_state
        .runtime
        .fan_states
        .iter()
        .map(|fan_state| {
            Row::new(vec![
                fan_state.index.to_string(),
                fan_state.speed.to_string(),
                fan_state.duty.to_string(),
                fan_state.control_policy.to_string(),
            ])
        })
        .collect::<Vec<_>>();

    let widths =
        [Constraint::Length(5), Constraint::Length(9), Constraint::Length(8), Constraint::Fill(1)];
    let table = Table::new(rows, widths)
        .header(
            Row::new(vec!["Index", "Speed (%)", "Duty (%)", "Policy"]).style(Style::new().yellow()),
        )
        .column_spacing(2)
        .block(block);

    frame.render_widget(table, area);
}

pub fn render_cooling_chart(frame: &mut Frame, data: &MonitorData, area: Rect) {
    let title = Line::from("Fan Curve".bold());
    let block = Block::bordered().title(title.left_aligned()).border_set(border::PLAIN);

    let mut curve_data = data
        .gpu_state
        .fan_curve
        .iter()
        .map(|(t, d)| (f64::from(*t), f64::from(*d)))
        .collect::<Vec<_>>();
    curve_data.sort_by(|(t1, _), (t2, _)| t2.total_cmp(t1));

    let temp = f64::from(data.gpu_state.runtime.device_temperature);

    let fans_data = data
        .gpu_state
        .runtime
        .fan_states
        .iter()
        .map(|fs| (temp, f64::from(fs.speed)))
        .collect::<Vec<_>>();

    let curve_dataset = Dataset::default()
        .graph_type(GraphType::Line)
        .name("Fan Curve")
        .marker(Marker::Braille)
        .style(Style::default().fg(Color::Yellow))
        .data(&curve_data);

    let fans_dataset = Dataset::default()
        .graph_type(GraphType::Scatter)
        .name("Fans")
        .style(Style::default().fg(Color::Blue))
        .marker(Marker::Dot)
        .data(&fans_data);

    let chart = Chart::new(vec![curve_dataset, fans_dataset])
        .block(block)
        .x_axis(
            Axis::default()
                .title("Temperature (C)")
                .style(Style::default().fg(Color::Gray))
                .labels(["10".bold(), "55".into(), "100".bold()])
                .bounds([10.0, 100.0]),
        )
        .y_axis(
            Axis::default()
                .title("Fan Speed (%)")
                .style(Style::default().fg(Color::Gray))
                .labels(["0".bold(), "50".into(), "100".bold()])
                .bounds([0.0, 100.0]),
        );

    frame.render_widget(chart, area);
}

impl Widget for SpecsBlock<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let title = Line::from("GPU Specs".bold());
        let block = Block::bordered().title(title.left_aligned()).border_set(border::PLAIN);

        let text = Text::from(vec![
            Line::from("Clock Speeds".to_string().yellow()),
            Line::from(format!(
                "{} MHz (graphics), {} MHz (memory), {} MHz (video), {} MHz (SM)",
                self.data.gpu_state.runtime.clock_speeds.graphics,
                self.data.gpu_state.runtime.clock_speeds.memory,
                self.data.gpu_state.runtime.clock_speeds.video,
                self.data.gpu_state.runtime.clock_speeds.streaming_multiprocessor,
            )),
            Line::from(""),
            Line::from("Memory".to_string().yellow()),
            Line::from(format!(
                "{} (used), {} (total)",
                convert(self.data.gpu_state.runtime.memory_info.used as _),
                convert(self.data.gpu_state.runtime.memory_info.total as _),
            )),
            Line::from(""),
            Line::from("Power Usage".to_string().yellow()),
            Line::from(format!("{:.3} W", self.data.gpu_state.runtime.power_usage,)),
            Line::from(""),
            Line::from("PCIe Connection".to_string().yellow()),
            Line::from(format!(
                "Current: {}x{} ({})",
                self.data.gpu_state.runtime.current_pcie_link.gen,
                self.data.gpu_state.runtime.current_pcie_link.width,
                convert(self.data.gpu_state.runtime.current_pcie_link.speed as _).replace('B', "T")
                    + "/s",
            )),
            Line::from(format!(
                "Maximum: {}x{} ({})",
                self.data.gpu_state.persistent.max_pcie_link.gen,
                self.data.gpu_state.persistent.max_pcie_link.width,
                convert(self.data.gpu_state.persistent.max_pcie_link.speed as _).replace('B', "T")
                    + "/s",
            )),
            Line::from(""),
            Line::from("Temperature Thresholds".to_string().yellow()),
            Line::from(format!(
                "{} C (shutdown), {} C (slowdown), {} C (gpumax)",
                self.data.gpu_state.persistent.temp_thresholds.shutdown,
                self.data.gpu_state.persistent.temp_thresholds.slowdown,
                self.data.gpu_state.persistent.temp_thresholds.gpumax
            )),
            Line::from(""),
            Line::from("Fan Speed Thresholds".to_string().yellow()),
            Line::from(format!(
                "{}% (min), {}% (max)",
                self.data.gpu_state.persistent.minmax_fan_speeds.min,
                self.data.gpu_state.persistent.minmax_fan_speeds.max,
            )),
        ]);

        Paragraph::new(text).block(block).render(area, buf);
    }
}

impl Widget for ErrorBlock<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let title = Line::from("Tjaele Monitor Error".bold());
        let block = Block::bordered().title(title.centered()).border_set(border::THICK);

        let mut lines =
            vec![Line::from(vec!["Tjaele Monitor failed to acquire GPU data with error chain: "
                .to_string()
                .yellow()])];

        for (i, e) in self.error.chain().enumerate() {
            lines.push(Line::from(vec![format!("[{i}]: {e}\n").into()]));
        }

        Paragraph::new(Text::from(lines)).block(block).render(area, buf);
    }
}
