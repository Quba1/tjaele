use std::ffi::OsStr;

use crate::{intermediate_bindings::MinMaxFanSpeeds, AdditionalNvmlFunctionality};
use anyhow::{bail, ensure, Context, Result};
use nvml_wrapper::{
    cuda_driver_version_major, cuda_driver_version_minor,
    enum_wrappers::device::{Clock, TemperatureSensor, TemperatureThreshold},
    enums::device::DeviceArchitecture,
    struct_wrappers::device::MemoryInfo,
    structs::device::CudaComputeCapability,
    Device, Nvml,
};
use ouroboros::self_referencing;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

pub struct GpuManager {
    // This is mutexed because I don't know if simulataneous calls to NVML is sound
    // Using Tokio mutex is somewhat justified as it's an IO function
    nvml_handle: Mutex<NvmlHandle>,
    persistent_params: PersistentGpuParams,
}

#[self_referencing]
struct NvmlHandle {
    nvml: Nvml,
    #[borrows(nvml)]
    #[covariant]
    device: Device<'this>,
}

impl GpuManager {
    pub fn init() -> Result<Self> {
        // recommended path for loading nvml
        let nvml = Nvml::builder().lib_path(OsStr::new("libnvidia-ml.so.1")).init()?;
        ensure!(
            nvml.device_count()? == 1,
            "nvmlcontrol currently supports platforms with one GPU only"
        );

        let nvml_handle =
            NvmlHandleTryBuilder { nvml, device_builder: |nvml: &Nvml| nvml.device_by_index(0) }
                .try_build()?;

        let persistent_params = PersistentGpuParams::init(&nvml_handle)?;

        Ok(GpuManager { nvml_handle: Mutex::new(nvml_handle), persistent_params })
    }

    pub async fn read_state(&self) -> Result<GpuState> {
        Ok(GpuState {
            runtime: RuntimeGpuParams::read_from_device(
                self.nvml_handle.lock().await.borrow_device(),
                self.persistent_params.num_fans,
            )?,
            persistent: self.persistent_params.clone(),
        })
    }
}

impl Drop for GpuManager {
    fn drop(&mut self) {
        tokio::task::block_in_place(|| {
            let handle = self.nvml_handle.blocking_lock();
            let device = handle.borrow_device();

            for fan_idx in 0..self.persistent_params.num_fans {
                device.set_default_fan_speed(fan_idx as u32)
                    // We panic here on purpose, so that failure "wreaks havoc"
                    // Ignoring error here could be potentially dangerous for the GPU
                    .expect("Failed to set auto fan control policy upon nvmlcontrol shutdown");
            }
        });
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GpuState {
    pub runtime: RuntimeGpuParams,
    pub persistent: PersistentGpuParams,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeGpuParams {
    pub current_pcie_link: PCIeLink,
    pub memory_info: MemoryInfo,
    pub power_usage: f64,
    pub device_temperature: u32,
    pub fan_states: Vec<FanState>,
    pub clock_speeds: ClockSpeeds,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistentGpuParams {
    pub sys_info: SysInfo,
    pub device_name: String,
    pub architecture: DeviceArchitecture,
    pub num_cores: u32,
    pub num_fans: usize,
    pub max_pcie_link: PCIeLink,
    pub temp_thresholds: GpuTemperatureThresholds,
    pub minmax_fan_speeds: MinMaxFanSpeeds,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SysInfo {
    pub cuda_version: CudaVersion,
    pub driver_version: String,
    pub cuda_capability: CudaComputeCapability,
    pub nvml_version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuTemperatureThresholds {
    pub shutdown: u32,
    pub slowdown: u32,
    pub gpumax: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PCIeLink {
    pub gen: u32,
    pub width: u32,
    pub speed: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClockSpeeds {
    pub memory: u32,
    pub graphics: u32,
    pub video: u32,
    pub streaming_multiprocessor: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CudaVersion {
    pub major: i32,
    pub minor: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FanState {
    pub index: usize,
    pub speed: u32,
    pub control_policy: u32,
}

impl PersistentGpuParams {
    pub(self) fn init(nvml_handle: &NvmlHandle) -> Result<Self> {
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
    pub(self) fn read_from_device(device: &Device, num_fans: usize) -> Result<Self> {
        Ok(RuntimeGpuParams {
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
                .with_context(|| format!("Failed to read fan_{index} policy"))?,
        })
    }
}
