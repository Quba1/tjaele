use crate::FanControlPolicy;

impl From<u32> for FanControlPolicy {
    fn from(value: u32) -> Self {
        match value {
            0 => Self::Automatic,
            1 => Self::Manual,
            _ => Self::Unknown,
        }
    }
}
