use super::{
    intermediate_bindings::AdditionalNvmlFunctionality, ouroboros_impl_nvml_handle::NvmlHandle,
};
use anyhow::{Context, Result};
use chrono::Local;
use nvml_wrapper::{
    cuda_driver_version_major, cuda_driver_version_minor,
    enum_wrappers::device::{Clock, TemperatureSensor, TemperatureThreshold},
};
use tjaele_types::{
    ClockSpeeds, CudaVersion, FanState, GpuTemperatureThresholds, PCIeLink, PersistentGpuParams,
    RuntimeGpuParams, SysInfo,
};

impl NvmlHandle {
    pub(super) fn read_persistent_params(&self) -> Result<PersistentGpuParams> {
        let device = self.borrow_device();

        Ok(PersistentGpuParams {
            sys_info: self.read_sys_info()?,

            device_name: device.name().context("Failed to read GPU name")?,
            architecture: device.architecture().context("Failed to read GPU arch")?.into(),
            num_cores: device.num_cores().context("Failed to read GPU num cores")?,
            num_fans: device.num_fans().context("Failed to read GPU num fans")? as usize,

            max_pcie_link: self.read_max_pcie_link().context("Failed to read GPU max PCIe link")?,

            temp_thresholds: GpuTemperatureThresholds {
                shutdown: device
                    .temperature_threshold(TemperatureThreshold::Shutdown)
                    .context("Failed to read GPU shutdown temperature")?,
                slowdown: device
                    .temperature_threshold(TemperatureThreshold::Slowdown)
                    .context("Failed to read GPU slowdown temperature")?,
                gpumax: device
                    .temperature_threshold(TemperatureThreshold::GpuMax)
                    .context("Failed to read GPU gpumax temperature")?,
            },

            minmax_fan_speeds: device
                .min_max_fan_speed()
                .context("Failed to read GPU min/max fan speeds")?,
        })
    }

    pub(super) fn read_runtime_params(&self, num_fans: usize) -> Result<RuntimeGpuParams> {
        let device = self.borrow_device();

        Ok(RuntimeGpuParams {
            probe_time: Local::now(),
            current_pcie_link: self
                .read_current_pcie_link()
                .context("Failed to read GPU PCIe link info")?,
            memory_info: device.memory_info().context("Failed to read GPU memory info")?.into(),
            power_usage: f64::from(device.power_usage().context("Failed to read GPU power usage")?)
                / 1000.0,
            clock_speeds: self.read_clock_speeds().context("Failed to read GPU clock speeds")?,
            device_temperature: device
                .temperature(TemperatureSensor::Gpu)
                .context("Failed to read GPU temperature")?,
            fan_states: (0..num_fans)
                .map(|index| -> Result<FanState> { self.read_fan_state(index) })
                .collect::<Result<Vec<_>>>()
                .context("Failed to read GPU fan states")?,
        })
    }

    fn read_sys_info(&self) -> Result<SysInfo> {
        let nvml = self.borrow_nvml();
        let device = self.borrow_device();

        Ok(SysInfo {
            driver_version: nvml.sys_driver_version()?,
            cuda_version: self.read_cuda_version()?,
            cuda_capability: device.cuda_compute_capability()?.into(),
            nvml_version: nvml.sys_nvml_version()?,
        })
    }

    fn read_cuda_version(&self) -> Result<CudaVersion> {
        let nvml = self.borrow_nvml();
        let cuda_version = nvml.sys_cuda_driver_version()?;

        Ok(CudaVersion {
            major: cuda_driver_version_major(cuda_version),
            minor: cuda_driver_version_minor(cuda_version),
        })
    }

    pub(self) fn read_max_pcie_link(&self) -> Result<PCIeLink> {
        let device = self.borrow_device();

        Ok(PCIeLink {
            gen: device.max_pcie_link_gen()?,
            width: device.max_pcie_link_width()?,
            speed: device
                .max_pcie_link_speed()?
                .as_integer()
                .map(u64::from)
                .map(|x| x * 1_000_000)
                .context("Couldn't convert PCIe max link speed")?,
        })
    }

    fn read_current_pcie_link(&self) -> Result<PCIeLink> {
        let device = self.borrow_device();

        Ok(PCIeLink {
            gen: device.current_pcie_link_gen()?,
            width: device.current_pcie_link_width()?,
            speed: device.pcie_link_speed().map(u64::from).map(|x| x * 1_000_000)?,
        })
    }

    fn read_clock_speeds(&self) -> Result<ClockSpeeds> {
        let device = self.borrow_device();

        Ok(ClockSpeeds {
            memory: device.clock_info(Clock::Memory)?,
            graphics: device.clock_info(Clock::Graphics)?,
            video: device.clock_info(Clock::Video)?,
            streaming_multiprocessor: device.clock_info(Clock::SM)?,
        })
    }

    fn read_fan_state(&self, index: usize) -> Result<FanState> {
        let device = self.borrow_device();

        Ok(FanState {
            index,
            speed: device
                .fan_speed(index as u32)
                .with_context(|| format!("Failed to read fan_{index} speed"))?,
            duty: device
                .fan_duty(index as u32)
                .with_context(|| format!("Failed to read fan_{index} duty"))?,
            control_policy: device
                .fan_control_policy(index as u32)
                .with_context(|| format!("Failed to read fan_{index} policy"))?
                .into(),
        })
    }
}
