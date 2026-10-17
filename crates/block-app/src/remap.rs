#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
pub(crate) use linux::*;

#[cfg(not(target_os = "linux"))]
pub(crate) fn load_cached(_cached: Option<Vec<u8>>) {}
