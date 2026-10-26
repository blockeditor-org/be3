use std::ffi::CStr;
use std::time::{Duration, Instant};

use beui_renderer_wgpu::present::OpenDevice;

use crate::{Frame, Target, View};

#[derive(Clone)]
pub struct Xr(());

impl Xr {
    pub fn open(_application: &str) -> Result<Self, String> {
        Err("OpenXR is not supported on this platform".to_owned())
    }

    pub fn runtime(&self) -> &str {
        ""
    }

    pub fn open_device(&self, extensions: &'static [&'static CStr]) -> OpenDevice {
        OpenDevice {
            extensions,
            gpu: None,
        }
    }

    pub fn headset(&self) -> Headset {
        Headset(())
    }
}

pub const RETRY: Duration = Duration::from_secs(2);

pub struct Headset(());

impl Headset {
    pub fn runtime(&self) -> &str {
        ""
    }

    pub fn running(&self) -> bool {
        false
    }

    pub fn shown(&self) -> u64 {
        0
    }

    pub fn lost(&self) -> bool {
        true
    }

    pub fn returns(&self) -> bool {
        false
    }

    pub fn reopen(&mut self, _device: &wgpu::Device) -> bool {
        false
    }

    pub fn relocate(&mut self) -> Option<Vec<View>> {
        None
    }

    pub fn pace(&mut self) -> Option<Instant> {
        None
    }

    pub fn begin(&mut self, _device: &wgpu::Device) -> Option<Frame> {
        None
    }

    pub fn submit(
        &mut self,
        queue: &wgpu::Queue,
        commands: wgpu::CommandEncoder,
        _draw: &mut dyn FnMut(&[Target<'_>], &mut wgpu::CommandEncoder),
    ) {
        queue.submit([commands.finish()]);
    }
}
