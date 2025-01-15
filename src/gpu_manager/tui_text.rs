use pretty_bytes::converter::convert;
use ratatui::{style::Stylize, text::Line};

use super::GpuState;

impl GpuState {
    pub fn render_tui_lines(&self) -> Vec<Line<'static>> {
        let mut lines = Vec::new();

        lines.push(Line::from(vec![
            "System Time: ".to_string().yellow(),
            self.runtime.probe_time.to_rfc2822().to_string().into(),
        ]));
        lines.push(Line::from(vec!["".into()]));

        lines.push(Line::from(vec![
            "Device: ".to_string().yellow(),
            format!(
                "{} (architecture: {}, CUDA cores: {}, fans: {})",
                self.persistent.device_name,
                self.persistent.architecture,
                self.persistent.num_cores,
                self.persistent.num_fans
            )
            .into(),
        ]));
        lines.push(Line::from(vec!["".into()]));

        lines.push(Line::from(vec![
            "Driver version: ".to_string().yellow(),
            self.persistent.sys_info.driver_version.to_string().into(),
        ]));
        lines.push(Line::from(vec![
            "CUDA Driver version: ".to_string().yellow(),
            format!(
                "{}.{}",
                self.persistent.sys_info.cuda_version.major,
                self.persistent.sys_info.cuda_version.minor
            )
            .into(),
        ]));
        lines.push(Line::from(vec![
            "CUDA Compute Capability: ".to_string().yellow(),
            format!(
                "{}.{}",
                self.persistent.sys_info.cuda_capability.major,
                self.persistent.sys_info.cuda_capability.minor
            )
            .into(),
        ]));
        lines.push(Line::from(vec![
            "NVML version: ".to_string().yellow(),
            self.persistent.sys_info.nvml_version.to_string().into(),
        ]));
        lines.push(Line::from(vec!["".into()]));

        lines.push(Line::from(vec![
            "Temperature thresholds: ".to_string().yellow(),
            format!(
                "{} C (shutdown), {} C (slowdown), {} C (gpumax)",
                self.persistent.temp_thresholds.shutdown,
                self.persistent.temp_thresholds.slowdown,
                self.persistent.temp_thresholds.gpumax
            )
            .into(),
        ]));
        lines.push(Line::from(vec![
            "Fan speed thresholds: ".to_string().yellow(),
            format!(
                "{}% (min), {}% (max)",
                self.persistent.minmax_fan_speeds.min, self.persistent.minmax_fan_speeds.max,
            )
            .into(),
        ]));
        lines.push(Line::from(vec!["".into()]));

        lines.push(Line::from(vec![
            "Current PCIe connection: ".to_string().yellow(),
            format!(
                "{}x{} ({})",
                self.runtime.current_pcie_link.gen,
                self.runtime.current_pcie_link.width,
                convert(self.runtime.current_pcie_link.speed as _).replace("B", "T") + "/s",
            )
            .into(),
        ]));
        lines.push(Line::from(vec![
            "Maximum PCIe connection: ".to_string().yellow(),
            format!(
                "{}x{} ({})",
                self.persistent.max_pcie_link.gen,
                self.persistent.max_pcie_link.width,
                convert(self.persistent.max_pcie_link.speed as _).replace("B", "T") + "/s",
            )
            .into(),
        ]));
        lines.push(Line::from(vec!["".into()]));

        lines.push(Line::from(vec![
            "Clock: ".to_string().yellow(),
            format!(
                "{} MHz (graphics), {} MHz (memory), {} MHz (video), {} MHz (SM)",
                self.runtime.clock_speeds.graphics,
                self.runtime.clock_speeds.memory,
                self.runtime.clock_speeds.video,
                self.runtime.clock_speeds.streaming_multiprocessor,
            )
            .into(),
        ]));
        lines.push(Line::from(vec![
            "Memory: ".to_string().yellow(),
            format!(
                "{} (used), {} (total)",
                convert(self.runtime.memory_info.used as _),
                convert(self.runtime.memory_info.total as _),
            )
            .into(),
        ]));
        lines.push(Line::from(vec![
            "Power usage: ".to_string().yellow(),
            format!("{:.3} W", self.runtime.power_usage,).into(),
        ]));
        lines.push(Line::from(vec!["".into()]));

        lines.push(Line::from(vec![
            "GPU Temperature: ".to_string().yellow(),
            format!("{} C", self.runtime.device_temperature).into(),
        ]));

        lines.push(Line::from(vec!["Fan speeds".to_string().yellow()]));
        self.runtime.fan_states.iter().for_each(|state| {
            lines.push(Line::from(vec![format!(
                "    fan_{} = {{speed: {}% policy: {}}}",
                state.index, state.speed, state.control_policy,
            )
            .into()]))
        });
        lines.push(Line::from(vec!["".into()]));

        lines
    }
}
