use std::collections::HashMap;

use smithay::backend::allocator::dmabuf::WeakDmabuf;
use smithay::backend::allocator::gbm::{GbmAllocator, GbmBufferFlags, GbmDevice};
use smithay::backend::allocator::{Format, Fourcc};
use smithay::backend::drm::{DrmDevice, DrmDeviceFd, GbmBufferedSurface};
use smithay::reexports::drm::control::{Device as _, ModeTypeFlags, connector, crtc};

use super::screen::{Frame, Screen};
use crate::gpu::{Usage, Vulkan};
use crate::render::Gpu;

type Swapchain = GbmBufferedSurface<GbmAllocator<DrmDeviceFd>, ()>;

pub struct Output {
    pub connector: connector::Handle,
    pub crtc: crtc::Handle,
    swapchain: Swapchain,
    pub screen: Screen,
    targets: HashMap<WeakDmabuf, wgpu::Texture>,
    flipping: bool,
}

pub fn connected(drm: &DrmDevice) -> Vec<(connector::Handle, connector::Info)> {
    let fd = drm.device_fd();
    let Ok(resources) = fd.resource_handles() else {
        return Vec::new();
    };
    resources
        .connectors()
        .iter()
        .filter_map(|handle| Some((*handle, fd.get_connector(*handle, true).ok()?)))
        .filter(|(_, info)| info.state() == connector::State::Connected && !info.modes().is_empty())
        .collect()
}

impl Output {
    pub fn new(
        drm: &mut DrmDevice,
        gbm: &GbmDevice<DrmDeviceFd>,
        gpu: &Gpu,
        connector: connector::Handle,
        info: &connector::Info,
        taken: &[crtc::Handle],
    ) -> Result<Self, String> {
        let mode = info
            .modes()
            .iter()
            .find(|mode| mode.mode_type().contains(ModeTypeFlags::PREFERRED))
            .or_else(|| info.modes().first())
            .copied()
            .ok_or("the connector has no modes")?;
        let fd = drm.device_fd();
        let resources = fd
            .resource_handles()
            .map_err(|error| format!("the device's resources could not be read: {error}"))?;
        let crtc = info
            .encoders()
            .iter()
            .filter_map(|encoder| fd.get_encoder(*encoder).ok())
            .flat_map(|encoder| resources.filter_crtcs(encoder.possible_crtcs()))
            .find(|crtc| !taken.contains(crtc))
            .ok_or("no display controller is free for the connector")?;
        let surface = drm
            .create_surface(crtc, mode, &[connector])
            .map_err(|error| format!("the output could not be set up: {error}"))?;
        let formats: Vec<Format> = gpu
            .vulkan()
            .map(|vulkan: &Vulkan| vulkan.formats(Usage::Render))
            .unwrap_or_default();
        let allocator = GbmAllocator::new(
            gbm.clone(),
            GbmBufferFlags::RENDERING | GbmBufferFlags::SCANOUT,
        );
        let swapchain = GbmBufferedSurface::new(
            surface,
            allocator,
            &[Fourcc::Xrgb8888, Fourcc::Argb8888],
            formats,
        )
        .map_err(|error| format!("the output's buffers could not be allocated: {error}"))?;
        let (width, height) = mode.size();
        let size = (u32::from(width), u32::from(height));
        Ok(Self {
            connector,
            crtc,
            swapchain,
            screen: Screen::new(gpu, size),
            targets: HashMap::new(),
            flipping: false,
        })
    }

    pub fn wants_frame(&self) -> bool {
        !self.flipping && self.screen.dirty()
    }

    pub fn replace_gpu(&mut self, gpu: &Gpu) {
        self.screen.replace_gpu(gpu);
        self.targets.clear();
    }

    pub fn reset(&mut self) {
        self.swapchain.reset_buffers();
        self.targets.clear();
        self.flipping = false;
        self.screen.invalidate();
    }

    pub fn flipped(&mut self) {
        let _ = self.swapchain.frame_submitted();
        self.flipping = false;
    }

    pub fn render(&mut self, gpu: &Gpu, frame: Frame<'_>) -> Result<(), String> {
        let (dmabuf, _) = self
            .swapchain
            .next_buffer()
            .map_err(|error| format!("no buffer is free to draw into: {error}"))?;
        let device = gpu.device();
        let target = match self.targets.get(&dmabuf.weak()) {
            Some(target) => target.clone(),
            None => {
                let vulkan = gpu.vulkan().ok_or("the device cannot import buffers")?;
                let imported = vulkan.import(device, &dmabuf, true, Usage::Render)?;
                self.targets.insert(dmabuf.weak(), imported.texture.clone());
                imported.texture
            }
        };
        let submitted = self.screen.draw(gpu, &frame, &target);
        device
            .poll(wgpu::PollType::Wait {
                submission_index: Some(submitted),
                timeout: None,
            })
            .map_err(|error| format!("the frame did not finish: {error}"))?;
        self.swapchain
            .queue_buffer(None, None, ())
            .map_err(|error| format!("the frame could not be shown: {error}"))?;
        self.flipping = true;
        Ok(())
    }
}
