mod impls;
#[cfg(feature = "nvml_types")]
mod nvml_integration;

use chrono::{DateTime, Local};
use derive_more::derive::Display;
use serde::{Deserialize, Serialize};

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
    pub memory_info: GpuMemStats,
    pub power_usage: f64,
    // there's only one temperature sensor variant
    pub device_temperature: u32,
    pub fan_states: Vec<FanState>,
    pub clock_speeds: ClockSpeeds,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistentGpuParams {
    pub sys_info: SysInfo,
    pub device_name: String,
    pub architecture: GpuArchitecture,
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

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct MinMaxFanSpeeds {
    pub min: u32,
    pub max: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuMemStats {
    pub free: u64,
    pub total: u64,
    pub used: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GpuArchitecture {
    Kepler,
    Maxwell,
    Pascal,
    Volta,
    Turing,
    Ampere,
    Ada,
    Hopper,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CudaComputeCapability {
    pub major: i32,
    pub minor: i32,
}
