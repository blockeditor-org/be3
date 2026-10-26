use std::ffi::CStr;
use std::time::{Duration, Instant};

use beui_core::app::Waker;
use beui_renderer_wgpu::present::OpenDevice;

use crate::{Fill, Frame, Quad, Status, Target, View};

const UNSUPPORTED: &str = "OpenXR is not supported on this platform";

#[derive(Clone)]
pub struct Link(());

impl Link {
    pub fn new(_application: &str) -> Self {
        eprintln!("beui-layer-xr: {UNSUPPORTED}");
        Self(())
    }

    pub fn status(&self) -> Status {
        Status::Failed(UNSUPPORTED.to_owned())
    }

    pub fn opened(&self) -> Option<Xr> {
        None
    }

    pub fn open_device(&self, extensions: &'static [&'static CStr]) -> OpenDevice {
        OpenDevice {
            extensions,
            gpu: None,
        }
    }

    pub fn set_waker(&self, _waker: Waker) {}

    pub fn lost(&self, _xr: &Xr) {}
}

#[derive(Clone)]
pub struct Xr(());

impl Xr {
    pub fn same(&self, _other: &Xr) -> bool {
        true
    }

    pub fn runtime(&self) -> &str {
        ""
    }

    pub fn headset(&self) -> Headset {
        Headset(Xr(()))
    }
}

pub const RETRY: Duration = Duration::from_secs(2);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Lost {
    Session,
    Instance,
    Ended,
}

pub struct Headset(Xr);

impl Headset {
    pub fn runtime(&self) -> &str {
        ""
    }

    pub fn xr(&self) -> &Xr {
        &self.0
    }

    pub fn running(&self) -> bool {
        false
    }

    pub fn shown(&self) -> u64 {
        0
    }

    pub fn lost(&self) -> Option<Lost> {
        Some(Lost::Ended)
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

    pub fn layered(&self) -> usize {
        0
    }

    pub fn submit(
        &mut self,
        _device: &wgpu::Device,
        queue: &wgpu::Queue,
        commands: wgpu::CommandEncoder,
        _quads: &[Quad],
        _draw: &mut dyn FnMut(&[Target<'_>], &[Fill<'_>], &mut wgpu::CommandEncoder),
    ) {
        queue.submit([commands.finish()]);
    }
}
