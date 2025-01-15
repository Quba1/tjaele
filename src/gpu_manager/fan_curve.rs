#![allow(clippy::cast_sign_loss)]

use std::{
    collections::hash_map::{
        Entry::{Occupied, Vacant},
        OccupiedEntry,
    },
    error::Error,
    fmt,
    hash::Hash,
};

use super::{GpuManager, TjaeleControlConfig};
use crate::gpu_manager::intermediate_bindings::AdditionalNvmlFunctionality;
use anyhow::{anyhow, ensure, Context, Result};
use nvml_wrapper::enum_wrappers::device::TemperatureSensor;
use rustc_hash::FxHashMap;
use tracing::debug;

impl GpuManager {
    /// Returns temperature used for setting duty
    pub async fn set_duty_with_curve(&self, previous_temp: u32) -> Result<u32> {
        let handle = self.nvml_handle.lock().await;
        let device = handle.borrow_device();

        let new_temp =
            device.temperature(TemperatureSensor::Gpu).context("Failed to read GPU temperature")?;

        let hysteresis_range = previous_temp
            .saturating_sub(u32::from(self.control_config.hysteresis))
            ..=previous_temp.saturating_add(u32::from(self.control_config.hysteresis));

        if hysteresis_range.contains(&new_temp) {
            debug!("Fan duty not changed - temperature within hysteresis ({new_temp})C");
            return Ok(previous_temp);
        }

        let temp_8bit =
            u8::try_from(new_temp).context("Your device somehow is warmer than 255C")?;
        let target_duty = *self
            .control_config
            .fan_curve
            .get(&temp_8bit)
            .context("Missing fan curve point - this should not happen")?;
        ensure!(target_duty <= 100, "Fan duty failed sanity check - this should not happen");

        for fan_idx in 0..self.persistent_params.num_fans {
            device
                .set_fan_speed(fan_idx as u32, u32::from(target_duty))
                .context("Failed to set fan speed")?;
        }

        debug!("Fan duty changed to {target_duty}%, temperature ({new_temp})C");

        Ok(new_temp)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct FanCurvePoint {
    temp: u8,
    duty: u8,
}

impl From<(&u8, &u8)> for FanCurvePoint {
    fn from(value: (&u8, &u8)) -> Self {
        FanCurvePoint { temp: *value.0, duty: *value.1 }
    }
}

impl TjaeleControlConfig {
    pub(super) fn precompute_fan_curve(mut self) -> Result<Self> {
        let mut anchor_points = self.fan_curve.iter().map(FanCurvePoint::from).collect::<Vec<_>>();
        anchor_points.sort_by_key(|pt| pt.temp);
        let anchor_points = anchor_points; // remove mutability

        // from 0 to first anchor we simply copy first duty (flat line)
        for temp in 0..anchor_points[0].temp {
            TryInsert::try_insert(&mut self.fan_curve, temp, anchor_points[0].duty)
                .map_err(|_| anyhow!("Found curve point which should not yet be present"))?;
        }

        // now we create a linear function between each pair and draw the curve
        for i in 0..anchor_points.len() - 1 {
            let lo_point = anchor_points[i];
            let hi_point = anchor_points[i + 1];

            ensure!(lo_point.duty <= hi_point.duty, "Fan duty must not decrease with temperature");

            let m = (f64::from(hi_point.duty) - f64::from(lo_point.duty))
                / (f64::from(hi_point.temp) - f64::from(lo_point.temp));
            let b = f64::from(lo_point.duty) - (m * f64::from(lo_point.temp));

            for temp in (lo_point.temp + 1)..hi_point.temp {
                let duty = (m * f64::from(temp) + b).ceil() as u8;
                TryInsert::try_insert(&mut self.fan_curve, temp, duty)
                    .map_err(|_| anyhow!("Found curve point which should not yet be present"))?;
            }
        }

        let last_point = *anchor_points.last().context("Last curve point not found")?;

        // from the last point to the end we again draw a flat line
        for temp in (last_point.temp + 1)..=u8::MAX {
            TryInsert::try_insert(&mut self.fan_curve, temp, last_point.duty)
                .map_err(|_| anyhow!("Found curve point which should not yet be present"))?;
        }

        self.validate_fan_curve()?;

        Ok(self)
    }

    fn validate_fan_curve(&self) -> Result<()> {
        let mut curve_points = self.fan_curve.iter().map(FanCurvePoint::from).collect::<Vec<_>>();
        curve_points.sort_by_key(|pt| pt.temp);

        for i in 0..curve_points.len() - 1 {
            let lo_point = curve_points[i];
            let hi_point = curve_points[i + 1];

            ensure!(lo_point.duty <= hi_point.duty, "Generated fun curve is not valid (direction)");
            ensure!(lo_point.duty <= 100, "Generated fun curve is not valid (fan duty)");
            ensure!(hi_point.duty <= 100, "Generated fun curve is not valid (fan duty)");
        }
        Ok(())
    }
}

// direct copy from std, because try_insert not stabilised still
trait TryInsert<K: Eq + Hash, V> {
    fn try_insert(&mut self, key: K, value: V) -> Result<&mut V, OccupiedError<'_, K, V>>;
}

#[derive(Debug)]
struct OccupiedError<'a, K: 'a, V: 'a> {
    /// The entry in the map that was already occupied.
    pub entry: OccupiedEntry<'a, K, V>,
    /// The value which was not inserted, because the entry was already occupied.
    pub value: V,
}

impl<K: fmt::Debug, V: fmt::Debug> Error for OccupiedError<'_, K, V> {
    #[allow(deprecated)]
    fn description(&self) -> &'static str {
        "key already exists"
    }
}

impl<K: fmt::Debug, V: fmt::Debug> fmt::Display for OccupiedError<'_, K, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "failed to insert {:?}, key {:?} already exists with value {:?}",
            self.value,
            self.entry.key(),
            self.entry.get(),
        )
    }
}

impl<K: Eq + Hash, V> TryInsert<K, V> for FxHashMap<K, V> {
    fn try_insert(&mut self, key: K, value: V) -> Result<&mut V, OccupiedError<'_, K, V>> {
        match self.entry(key) {
            Occupied(entry) => Err(OccupiedError { entry, value }),
            Vacant(entry) => Ok(entry.insert(value)),
        }
    }
}
