use std::{ffi::OsStr, fmt::Debug, path::Path, time::Duration};

mod device_probe;
mod fan_curve;
mod intermediate_bindings;

use anyhow::{ensure, Result};
use intermediate_bindings::AdditionalNvmlFunctionality;
use nvml_wrapper::{Device, Nvml};
use ouroboros::self_referencing;
use rustc_hash::FxHashMap;
use serde::Deserialize;
use serde_with::serde_as;
use tjaele_types::{GpuState, PersistentGpuParams};
use tracing::info;

#[derive(Debug)]
pub struct GpuManager {
    nvml_handle: NvmlHandle,
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

        let persistent_params = nvml_handle.read_persistent_params()?;

        Ok(GpuManager { nvml_handle, persistent_params, control_config })
    }

    pub fn read_state(&self) -> Result<GpuState> {
        Ok(GpuState {
            runtime: self.nvml_handle.read_runtime_params(self.persistent_params.num_fans)?,
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
        let device = self.nvml_handle.borrow_device();

        for fan_idx in 0..self.persistent_params.num_fans {
            device.set_default_fan_speed(fan_idx as u32)
                    // We panic here on purpose, so that failure "wreaks havoc"
                    // Ignoring error here could be potentially dangerous for the GPU
                    .expect("Failed to set auto fan control policy upon nvmlcontrol shutdown");
        }
        info!("All fans policy set to automatic");
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
