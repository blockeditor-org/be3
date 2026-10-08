use std::collections::HashMap;
use std::os::fd::OwnedFd;

use rustix::event::{PollFd, PollFlags, Timespec, poll};
use smithay::backend::allocator::dmabuf::WeakDmabuf;
use smithay::backend::allocator::gbm::{GbmAllocator, GbmBufferFlags, GbmDevice};
use smithay::backend::allocator::{Format, Fourcc};
use smithay::backend::drm::{DrmDevice, DrmDeviceFd, GbmBufferedSurface};
use smithay::backend::renderer::sync::{Fence, Interrupted, SyncPoint};
use smithay::reexports::drm::control::{
    Device as _, Mode, ModeFlags, ModeTypeFlags, connector, crtc,
};
use smithay::reexports::drm::{Device as _, DriverCapability};

use super::screen::{Frame, Screen};
use be_dmabuf::{Usage, Vulkan};

use crate::display::{DisplayConfig, DisplayMode, Monitor};
use crate::gpu::{Gpu, Submitted};
use crate::modes::{
    Candidate, Timing, choose, default_mode, identity, listed, monitor_id, monitor_name,
    preferred_mode, refresh_millihertz,
};

type Swapchain = GbmBufferedSurface<GbmAllocator<DrmDeviceFd>, ()>;

enum State {
    Idle,
    Drawing(Option<OwnedFd>),
    Flipping,
}

pub struct Output {
    pub connector: connector::Handle,
    pub crtc: crtc::Handle,
    swapchain: Swapchain,
    pub screen: Screen,
    targets: HashMap<WeakDmabuf, wgpu::Texture>,
    modes: Vec<Mode>,
    candidates: Vec<Candidate>,
    chosen: usize,
    id: String,
    name: String,
    connector_name: String,
    fencing: bool,
    state: State,
    frame: u64,
    blanked: bool,
    dark: bool,
    darken_failed: bool,
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

fn edid(drm: &DrmDevice, connector: connector::Handle) -> Option<Vec<u8>> {
    let fd = drm.device_fd();
    let properties = fd.get_properties(connector).ok()?;
    properties.iter().find_map(|(handle, value)| {
        let info = fd.get_property(*handle).ok()?;
        if info.name().to_bytes() != b"EDID" {
            return None;
        }
        let blob = info.value_type().convert_value(*value).as_blob()?;
        fd.get_property_blob(blob).ok()
    })
}

fn candidate(mode: &Mode) -> Candidate {
    let (width, height) = mode.size();
    let (_, _, htotal) = mode.hsync();
    let (_, _, vtotal) = mode.vsync();
    let flags = mode.flags();
    let interlaced = flags.contains(ModeFlags::INTERLACE);
    Candidate {
        mode: DisplayMode {
            width: u32::from(width),
            height: u32::from(height),
            refresh_millihertz: refresh_millihertz(&Timing {
                clock_khz: mode.clock(),
                htotal,
                vtotal,
                vscan: mode.vscan(),
                interlaced,
                double_scan: flags.contains(ModeFlags::DBLSCAN),
            }),
        },
        preferred: mode.mode_type().contains(ModeTypeFlags::PREFERRED),
        interlaced,
    }
}

fn size_of(mode: &Mode) -> (u32, u32) {
    let (width, height) = mode.size();
    (u32::from(width), u32::from(height))
}

#[derive(Debug)]
struct SyncFile(OwnedFd);

pub fn wait_for(fence: &OwnedFd) -> Result<(), Interrupted> {
    let mut polled = [PollFd::new(fence, PollFlags::IN)];
    loop {
        match poll(&mut polled, None) {
            Ok(_) => return Ok(()),
            Err(rustix::io::Errno::INTR) => continue,
            Err(_) => return Err(Interrupted),
        }
    }
}

impl Fence for SyncFile {
    fn is_signaled(&self) -> bool {
        let mut polled = [PollFd::new(&self.0, PollFlags::IN)];
        let zero = Timespec {
            tv_sec: 0,
            tv_nsec: 0,
        };
        matches!(poll(&mut polled, Some(&zero)), Ok(count) if count > 0)
    }

    fn wait(&self) -> Result<(), Interrupted> {
        wait_for(&self.0)
    }

    fn is_exportable(&self) -> bool {
        true
    }

    fn export(&self) -> Option<OwnedFd> {
        self.0.try_clone().ok()
    }
}

impl Output {
    pub fn new(
        drm: &mut DrmDevice,
        gbm: &GbmDevice<DrmDeviceFd>,
        gpu: &Gpu,
        connector: connector::Handle,
        info: &connector::Info,
        taken: &[crtc::Handle],
        config: &DisplayConfig,
    ) -> Result<Self, String> {
        let connector_name = info.to_string();
        let identity = edid(drm, connector).as_deref().and_then(identity);
        let id = monitor_id(identity.as_ref(), &connector_name);
        let modes = info.modes().to_vec();
        let candidates: Vec<Candidate> = modes.iter().map(candidate).collect();
        let chosen = choose(&candidates, config.mode(&id)).ok_or("the connector has no modes")?;
        let mode = modes[chosen];
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
        let syncobj = fd
            .get_driver_capability(DriverCapability::SyncObj)
            .is_ok_and(|value| value != 0);
        let surface = drm
            .create_surface(crtc, mode, &[connector])
            .map_err(|error| format!("the output could not be set up: {error}"))?;
        let fencing = syncobj && !surface.is_legacy();
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
        Ok(Self {
            connector,
            crtc,
            swapchain,
            screen: Screen::new(gpu, size_of(&mode)),
            targets: HashMap::new(),
            modes,
            candidates,
            chosen,
            id,
            name: monitor_name(identity.as_ref(), &connector_name),
            connector_name,
            fencing,
            state: State::Idle,
            frame: 0,
            blanked: false,
            dark: false,
            darken_failed: false,
        })
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn monitor(&self) -> Monitor {
        let default = default_mode(&self.candidates).unwrap_or(self.chosen);
        let preferred = preferred_mode(&self.candidates).unwrap_or(default);
        Monitor {
            id: self.id.clone(),
            name: self.name.clone(),
            connector: self.connector_name.clone(),
            modes: listed(&self.candidates),
            default: self.candidates[default].mode,
            preferred: self.candidates[preferred].mode,
            current: self.candidates[self.chosen].mode,
        }
    }

    pub fn drawing(&self) -> bool {
        matches!(self.state, State::Drawing(_))
    }

    pub fn set_mode(&mut self, gpu: &Gpu, wanted: Option<DisplayMode>) -> bool {
        let Some(chosen) = choose(&self.candidates, wanted) else {
            return false;
        };
        if chosen == self.chosen || self.drawing() {
            return false;
        }
        let mode = self.modes[chosen];
        if let Err(error) = self.swapchain.use_mode(mode) {
            eprintln!(
                "beui: {} could not switch to {:?}: {error}",
                self.connector_name, self.candidates[chosen].mode
            );
            return false;
        }
        self.chosen = chosen;
        self.targets.clear();
        let rect = self.screen.rect;
        self.screen = Screen::new(gpu, size_of(&mode));
        self.screen.rect = rect;
        true
    }

    pub fn wants_frame(&self) -> bool {
        !self.blanked && matches!(self.state, State::Idle) && self.screen.dirty()
    }

    pub fn set_blanked(&mut self, blanked: bool) {
        if blanked == self.blanked {
            return;
        }
        self.blanked = blanked;
        if blanked {
            self.darken();
        } else {
            self.dark = false;
            self.darken_failed = false;
            self.screen.invalidate();
        }
    }

    pub fn lit_while_blanked(&self) -> bool {
        self.blanked && !self.dark
    }

    pub fn darken(&mut self) {
        if !self.blanked || self.dark || !matches!(self.state, State::Idle) {
            return;
        }
        match self.swapchain.surface().clear() {
            Ok(()) => self.dark = true,
            Err(error) if !self.darken_failed => {
                self.darken_failed = true;
                eprintln!(
                    "beui: {} could not be turned off, so it is tried again: {error}",
                    self.connector_name
                );
            }
            Err(_) => {}
        }
    }

    pub fn reset(&mut self) {
        self.swapchain.reset_buffers();
        self.targets.clear();
        self.state = State::Idle;
        self.frame += 1;
        self.screen.invalidate();
        self.dark = false;
        self.darken();
    }

    pub fn flipped(&mut self) {
        let _ = self.swapchain.frame_submitted();
        if matches!(self.state, State::Flipping) {
            self.state = State::Idle;
            self.darken();
        }
    }

    pub fn take_fence(&mut self) -> Option<(u64, OwnedFd)> {
        match &mut self.state {
            State::Drawing(fence) => Some((self.frame, fence.take()?)),
            _ => None,
        }
    }

    pub fn drawn(&mut self, frame: u64) {
        if frame != self.frame || !self.drawing() {
            return;
        }
        self.queue(None);
    }

    fn queue(&mut self, sync: Option<SyncPoint>) {
        match self.swapchain.queue_buffer(sync, None, ()) {
            Ok(()) => self.state = State::Flipping,
            Err(error) => {
                eprintln!("beui: the frame could not be shown: {error}");
                self.state = State::Idle;
                self.darken();
            }
        }
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
                let imported = vulkan.import(device, &dmabuf, false, Usage::Render)?;
                self.targets.insert(dmabuf.weak(), imported.texture.clone());
                imported.texture
            }
        };
        let commands = self.screen.draw(gpu, &frame, &target);
        self.frame += 1;
        match gpu.submit(commands) {
            Submitted::Fence(Some(fence)) if self.fencing => {
                self.queue(Some(SyncPoint::from(SyncFile(fence))));
            }
            Submitted::Fence(Some(fence)) => self.state = State::Drawing(Some(fence)),
            Submitted::Fence(None) => self.queue(None),
            Submitted::Index(submitted) => {
                device
                    .poll(wgpu::PollType::Wait {
                        submission_index: Some(submitted),
                        timeout: None,
                    })
                    .map_err(|error| format!("the frame did not finish: {error}"))?;
                self.queue(None);
            }
        }
        Ok(())
    }
}
