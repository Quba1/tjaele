use std::ffi::c_uint;

use nvml_wrapper::{
    error::{nvml_sym, nvml_try, NvmlError},
    Device,
};
use nvml_wrapper_sys::bindings::nvmlFanControlPolicy_t;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct MinMaxFanSpeeds {
    pub min: u32,
    pub max: u32,
}

pub trait AdditionalNvmlFunctionality {
    fn min_max_fan_speed(&self) -> Result<MinMaxFanSpeeds, NvmlError>;
    fn fan_control_policy(&self, fan_idx: u32) -> Result<u32, NvmlError>;
    fn set_fan_speed(&self, fan_idx: u32, fan_speed: u32) -> Result<(), NvmlError>;
    fn set_default_fan_speed(&self, fan_idx: u32) -> Result<(), NvmlError>;
}

impl AdditionalNvmlFunctionality for Device<'_> {
    fn min_max_fan_speed(&self) -> Result<MinMaxFanSpeeds, NvmlError> {
        let sym = nvml_sym(self.nvml().nvml_lib().nvmlDeviceGetMinMaxFanSpeed.as_ref())?;

        let mut min_speed: c_uint = 0;
        let mut max_speed: c_uint = 0;

        unsafe { nvml_try(sym(self.handle(), &mut min_speed, &mut max_speed))? }

        Ok(MinMaxFanSpeeds { min: min_speed, max: max_speed })
    }

    fn fan_control_policy(&self, fan_idx: u32) -> Result<u32, NvmlError> {
        let sym = nvml_sym(self.nvml().nvml_lib().nvmlDeviceGetFanControlPolicy_v2.as_ref())?;

        let mut policy: nvmlFanControlPolicy_t = 0;

        unsafe { nvml_try(sym(self.handle(), fan_idx, &mut policy))? }

        Ok(policy)
    }

    /// Disables automatic fan control and sets provided fan speed
    /// Fan speed must be between 0-100. This function does not check provided input.
    fn set_fan_speed(&self, fan_idx: u32, fan_speed: u32) -> Result<(), NvmlError> {
        let sym = nvml_sym(self.nvml().nvml_lib().nvmlDeviceSetFanSpeed_v2.as_ref())?;

        unsafe { nvml_try(sym(self.handle(), fan_idx, fan_speed)) }
    }

    /// Enables automatic fan control
    fn set_default_fan_speed(&self, fan_idx: u32) -> Result<(), NvmlError> {
        let sym = nvml_sym(self.nvml().nvml_lib().nvmlDeviceSetDefaultFanSpeed_v2.as_ref())?;

        unsafe { nvml_try(sym(self.handle(), fan_idx)) }
    }
}
