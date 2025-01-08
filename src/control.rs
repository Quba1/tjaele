use anyhow::Result;
use nvml_wrapper::{Device, Nvml};

use crate::intermediate_bindings::AdditionalNvmlFunctionality;

pub fn control_main(nvml: &Nvml) -> Result<()> {
    // must have histeresis

    let controller = FanController::init(nvml)?;

    Ok(())
}

// implementation is moved into struct to have a destructor called on program shutdown
#[derive(Debug)]
struct FanController<'nvml> {
    device: Device<'nvml>,
    num_fans: u32,
}

impl<'nvml> FanController<'nvml> {
    pub fn init(nvml: &'nvml Nvml) -> Result<Self> {
        let device = nvml.device_by_index(0)?;
        let num_fans = device.num_fans()?;

        Ok(Self { device, num_fans })
    }
}

impl<'nvml> Drop for FanController<'nvml> {
    fn drop(&mut self) {
        for fan_idx in 0..self.num_fans {
            self.device
                .set_default_fan_speed(fan_idx)
                // We panic here on purpose, so that failure "wreaks havoc"
                // Ignoring error here could be potentially dangerous for the GPU
                .expect("Failed to set auto fan control policy upon nvmlcontrol shutdown")
        }
    }
}
