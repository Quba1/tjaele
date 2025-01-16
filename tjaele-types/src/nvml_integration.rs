use nvml_wrapper::{
    enums::device::DeviceArchitecture, struct_wrappers::device::MemoryInfo,
    structs::device::CudaComputeCapability,
};

use crate::{GpuArchitecture, GpuMemStats};

impl From<MemoryInfo> for GpuMemStats {
    fn from(value: MemoryInfo) -> Self {
        GpuMemStats { free: value.free, total: value.total, used: value.used }
    }
}

impl From<DeviceArchitecture> for GpuArchitecture {
    fn from(value: DeviceArchitecture) -> Self {
        match value {
            DeviceArchitecture::Kepler => GpuArchitecture::Kepler,
            DeviceArchitecture::Maxwell => GpuArchitecture::Maxwell,
            DeviceArchitecture::Pascal => GpuArchitecture::Pascal,
            DeviceArchitecture::Volta => GpuArchitecture::Volta,
            DeviceArchitecture::Turing => GpuArchitecture::Turing,
            DeviceArchitecture::Ampere => GpuArchitecture::Ampere,
            DeviceArchitecture::Ada => GpuArchitecture::Ada,
            DeviceArchitecture::Hopper => GpuArchitecture::Hopper,
            DeviceArchitecture::Unknown => GpuArchitecture::Unknown,
        }
    }
}

impl From<CudaComputeCapability> for crate::CudaComputeCapability {
    fn from(value: CudaComputeCapability) -> Self {
        crate::CudaComputeCapability { major: value.major, minor: value.minor }
    }
}
