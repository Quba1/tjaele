use std::{ffi::OsStr, fmt::Debug, path::Path, time::Duration};

mod device_probe;
mod fan_curve;
mod intermediate_bindings;

use anyhow::{ensure, Result};
use chrono::{DateTime, Local};
use derive_more::derive::Display;
use intermediate_bindings::{AdditionalNvmlFunctionality, MinMaxFanSpeeds};
use nvml_wrapper::{
    enums::device::DeviceArchitecture, struct_wrappers::device::MemoryInfo,
    structs::device::CudaComputeCapability, Device, Nvml,
};
use ouroboros::self_referencing;
use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use tokio::sync::Mutex;
use tracing::info;

#[derive(Debug)]
pub struct GpuManager {
    // This is mutexed because I don't know if simulataneous calls to NVML is sound
    // Using Tokio mutex is somewhat justified as it's an IO function
    nvml_handle: Mutex<NvmlHandle>,
    persistent_params: PersistentGpuParams,
    pub control_config: TjaeleControlConfig,
}

#[self_referencing]
struct NvmlHandle {
    nvml: Nvml,
    #[borrows(nvml)]
    #[covariant]
    device: Device<'this>,
}

impl GpuManager {
    pub fn init<P: AsRef<Path> + Debug>(config_path: P) -> Result<Self> {
        let control_config =
            TjaeleControlConfig::new_from_file(config_path)?.precompute_fan_curve()?;

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

        Ok(GpuManager { nvml_handle: Mutex::new(nvml_handle), persistent_params, control_config })
    }

    pub async fn read_state(&self) -> Result<GpuState> {
        Ok(GpuState {
            runtime: RuntimeGpuParams::read_from_device(
                self.nvml_handle.lock().await.borrow_device(),
                self.persistent_params.num_fans,
            )?,
            persistent: self.persistent_params.clone(),
            fan_curve: self.control_config.fan_curve.iter().map(|(t, d)| (*t, *d)).collect(),
        })
    }

    pub async fn sleep(&self) {
        tokio::time::sleep(self.control_config.response_time).await;
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

impl Debug for NvmlHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NvmlHandle")
            .field("nvml", &self.borrow_nvml())
            .field("device", &self.borrow_device())
            .finish()
    }
}

#[serde_as]
#[derive(Debug, Clone, Deserialize)]
pub struct TjaeleControlConfig {
    #[serde_as(as = "serde_with::DurationSecondsWithFrac<f64>")]
    pub response_time: Duration,
    pub hysteresis: u16,
    #[serde_as(as = "Vec<(_, _)>")]
    pub fan_curve: FxHashMap<u8, u8>,
}

impl TjaeleControlConfig {
    pub(self) fn new_from_file<Q: AsRef<Path> + Debug>(path: Q) -> Result<Self> {
        let cfg = std::fs::read_to_string(&path)?;
        let cfg: Self = toml::from_str(&cfg)?;

        ensure!(cfg.hysteresis > 0 && cfg.hysteresis <= 5, "Hysteresis must be between 1C and 5C");
        ensure!(
            cfg.response_time.as_secs_f64() >= 0.25,
            "Response time must be at least than 0.25 seconds"
        );

        cfg.fan_curve.iter().try_for_each(|(_, &fan_duty)| -> Result<()> {
            ensure!(fan_duty <= 100, "Fan duty cannot be higher than 100%");
            Ok(())
        })?;

        ensure!(cfg.fan_curve.len() >= 3, "Fan curve must have at least 3 points");

        info!("Config loaded from {path:?}");

        Ok(cfg)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GpuState {
    pub runtime: RuntimeGpuParams,
    pub persistent: PersistentGpuParams,
    pub fan_curve: Vec<(u8, u8)>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeGpuParams {
    pub probe_time: DateTime<Local>,
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
    /// Actual fan speed
    pub speed: u32,
    /// Speed fan is set to
    pub duty: u32,
    pub control_policy: FanControlPolicy,
}

#[derive(Debug, Clone, Serialize, Deserialize, Display)]
pub enum FanControlPolicy {
    Automatic,
    Manual,
    Unknown,
}

impl From<u32> for FanControlPolicy {
    fn from(value: u32) -> Self {
        match value {
            0 => Self::Automatic,
            1 => Self::Manual,
            _ => Self::Unknown,
        }
    }
}
