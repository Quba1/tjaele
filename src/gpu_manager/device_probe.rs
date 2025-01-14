use super::{
    intermediate_bindings::AdditionalNvmlFunctionality, ouroboros_impl_nvml_handle::NvmlHandle,
    ClockSpeeds, CudaVersion, FanState, GpuTemperatureThresholds, PCIeLink, PersistentGpuParams,
    RuntimeGpuParams, SysInfo,
};
use anyhow::{Context, Result};
use chrono::Local;
use nvml_wrapper::{
    cuda_driver_version_major, cuda_driver_version_minor,
    enum_wrappers::device::{Clock, TemperatureSensor, TemperatureThreshold},
    Device, Nvml,
};

impl PersistentGpuParams {
    pub(super) fn init(nvml_handle: &NvmlHandle) -> Result<Self> {
        let nvml = nvml_handle.borrow_nvml();
        let device = nvml_handle.borrow_device();

        Ok(PersistentGpuParams {
            sys_info: SysInfo::read_from_driver(nvml, device)?,

            device_name: device.name().context("Failed to read GPU name")?,
            architecture: device.architecture().context("Failed to read GPU arch")?,
            num_cores: device.num_cores().context("Failed to read GPU num cores")?,
            num_fans: device.num_fans().context("Failed to read GPU num fans")? as usize,

            max_pcie_link: PCIeLink::max_from_device(device)
                .context("Failed to read GPU max PCIe link")?,

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
}

impl RuntimeGpuParams {
    pub(super) fn read_from_device(device: &Device, num_fans: usize) -> Result<Self> {
        Ok(RuntimeGpuParams {
            probe_time: Local::now(),
            current_pcie_link: PCIeLink::current_from_device(device)
                .context("Failed to read GPU PCIe link info")?,
            memory_info: device.memory_info().context("Failed to read GPU memory info")?,
            power_usage: f64::from(device.power_usage().context("Failed to read GPU power usage")?)
                / 1000.0,
            clock_speeds: ClockSpeeds::read_from_device(device)
                .context("Failed to read GPU clock speeds")?,
            // there's only one temperature sensor variant
            device_temperature: device
                .temperature(TemperatureSensor::Gpu)
                .context("Failed to read GPU temperature")?,
            fan_states: (0..num_fans)
                .map(|index| -> Result<FanState> { FanState::read_from_device(index, device) })
                .collect::<Result<Vec<_>>>()
                .context("Failed to read GPU fan states")?,
        })
    }
}

impl SysInfo {
    pub fn read_from_driver(nvml: &Nvml, device: &Device) -> Result<Self> {
        Ok(SysInfo {
            driver_version: nvml.sys_driver_version()?,
            cuda_version: CudaVersion::read_from_driver(nvml)?,
            cuda_capability: device.cuda_compute_capability()?,
            nvml_version: nvml.sys_nvml_version()?,
        })
    }
}

impl CudaVersion {
    pub fn read_from_driver(nvml: &Nvml) -> Result<Self> {
        let cuda_version = nvml.sys_cuda_driver_version()?;

        Ok(CudaVersion {
            major: cuda_driver_version_major(cuda_version),
            minor: cuda_driver_version_minor(cuda_version),
        })
    }
}

impl PCIeLink {
    pub(self) fn max_from_device(device: &Device) -> Result<Self> {
        Ok(PCIeLink {
            gen: device.max_pcie_link_gen()?,
            width: device.max_pcie_link_width()?,
            speed: device
                .max_pcie_link_speed()?
                .as_integer()
                .map(u64::from)
                .map(|x| x * 1000000)
                .context("Couldn't convert PCIe max link speed")?,
        })
    }

    pub(self) fn current_from_device(device: &Device) -> Result<Self> {
        Ok(PCIeLink {
            gen: device.current_pcie_link_gen()?,
            width: device.current_pcie_link_width()?,
            speed: device.pcie_link_speed().map(u64::from).map(|x| x * 1000000)?,
        })
    }
}

impl ClockSpeeds {
    pub(self) fn read_from_device(device: &Device) -> Result<Self> {
        Ok(ClockSpeeds {
            memory: device.clock_info(Clock::Memory)?,
            graphics: device.clock_info(Clock::Graphics)?,
            video: device.clock_info(Clock::Video)?,
            streaming_multiprocessor: device.clock_info(Clock::SM)?,
        })
    }
}

impl FanState {
    pub(self) fn read_from_device(index: usize, device: &Device) -> Result<Self> {
        Ok(FanState {
            index,
            speed: device
                .fan_speed(index as u32)
                .with_context(|| format!("Failed to read fan_{index} speed"))?,
            control_policy: device
                .fan_control_policy(index as u32)
                .with_context(|| format!("Failed to read fan_{index} policy"))?
                .into(),
        })
    }
}
