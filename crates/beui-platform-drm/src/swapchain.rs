use smithay::backend::allocator::dmabuf::{AsDmabuf, Dmabuf};
use smithay::backend::allocator::gbm::{GbmAllocator, GbmBuffer};
use smithay::backend::allocator::{Buffer, Format, Fourcc, Modifier, Slot, Swapchain as Slots};
use smithay::backend::drm::{
    DrmDeviceFd, DrmSurface, GbmBufferedSurface, PlaneConfig, PlaneDamageClips, PlaneState,
};
use smithay::backend::renderer::sync::SyncPoint;
use smithay::reexports::drm::buffer::{self, DrmFourcc, DrmModifier, PlanarBuffer};
use smithay::reexports::drm::control::{Device as _, FbCmd2Flags, Mode, framebuffer};
use smithay::utils::{Rectangle, Transform};

use crate::screen::Damage;

const COLOR_FORMATS: [Fourcc; 2] = [Fourcc::Xrgb8888, Fourcc::Argb8888];

pub enum Swapchain {
    Shared(GbmBufferedSurface<GbmAllocator<DrmDeviceFd>, ()>),
    Imported(Imported),
}

impl Swapchain {
    pub fn shared(
        surface: DrmSurface,
        allocator: GbmAllocator<DrmDeviceFd>,
        formats: Vec<Format>,
    ) -> Result<Self, String> {
        GbmBufferedSurface::new(surface, allocator, &COLOR_FORMATS, formats)
            .map(Self::Shared)
            .map_err(|error| format!("the output's buffers could not be allocated: {error}"))
    }

    pub fn imported(
        surface: DrmSurface,
        allocator: GbmAllocator<DrmDeviceFd>,
        formats: &[Format],
    ) -> Result<Self, String> {
        Imported::new(surface, allocator, formats).map(Self::Imported)
    }

    pub fn is_legacy(&self) -> bool {
        match self {
            Self::Shared(swapchain) => swapchain.surface().is_legacy(),
            Self::Imported(imported) => imported.surface.is_legacy(),
        }
    }

    pub fn use_mode(&mut self, mode: Mode) -> Result<(), String> {
        match self {
            Self::Shared(swapchain) => swapchain.use_mode(mode).map_err(|error| error.to_string()),
            Self::Imported(imported) => imported.use_mode(mode),
        }
    }

    pub fn clear(&self) -> Result<(), String> {
        let surface = match self {
            Self::Shared(swapchain) => swapchain.surface(),
            Self::Imported(imported) => &imported.surface,
        };
        surface.clear().map_err(|error| error.to_string())
    }

    pub fn frame_submitted(&mut self) {
        match self {
            Self::Shared(swapchain) => {
                let _ = swapchain.frame_submitted();
            }
            Self::Imported(imported) => imported.frame_submitted(),
        }
    }

    pub fn next_buffer(&mut self) -> Result<Dmabuf, String> {
        match self {
            Self::Shared(swapchain) => swapchain
                .next_buffer()
                .map(|(dmabuf, _)| dmabuf)
                .map_err(|error| error.to_string()),
            Self::Imported(imported) => imported.next_buffer(),
        }
    }

    pub fn queue_buffer(&mut self, sync: Option<SyncPoint>, damage: Damage) -> Result<(), String> {
        match self {
            Self::Shared(swapchain) => swapchain
                .queue_buffer(sync, damage, ())
                .map_err(|error| error.to_string()),
            Self::Imported(imported) => imported.queue_buffer(sync, damage),
        }
    }
}

pub struct Imported {
    surface: DrmSurface,
    slots: Slots<GbmAllocator<DrmDeviceFd>>,
    next: Option<Slot<GbmBuffer>>,
    pending: Option<Slot<GbmBuffer>>,
    current: Option<Slot<GbmBuffer>>,
}

struct Framebuffer {
    drm: DrmDeviceFd,
    fb: framebuffer::Handle,
    handles: Vec<buffer::Handle>,
}

impl Drop for Framebuffer {
    fn drop(&mut self) {
        let _ = self.drm.destroy_framebuffer(self.fb);
        close(&self.drm, &self.handles);
    }
}

fn close(drm: &DrmDeviceFd, handles: &[buffer::Handle]) {
    for handle in handles {
        let _ = drm.close_buffer(*handle);
    }
}

struct Planes<'a> {
    dmabuf: &'a Dmabuf,
    handles: [Option<buffer::Handle>; 4],
}

impl PlanarBuffer for Planes<'_> {
    fn size(&self) -> (u32, u32) {
        let size = self.dmabuf.size();
        (size.w as u32, size.h as u32)
    }

    fn format(&self) -> DrmFourcc {
        self.dmabuf.format().code
    }

    fn modifier(&self) -> Option<DrmModifier> {
        Some(self.dmabuf.format().modifier).filter(|modifier| *modifier != Modifier::Invalid)
    }

    fn pitches(&self) -> [u32; 4] {
        let mut pitches = [0; 4];
        for (pitch, stride) in pitches.iter_mut().zip(self.dmabuf.strides()) {
            *pitch = stride;
        }
        pitches
    }

    fn handles(&self) -> [Option<buffer::Handle>; 4] {
        self.handles
    }

    fn offsets(&self) -> [u32; 4] {
        let mut offsets = [0; 4];
        for (offset, plane) in offsets.iter_mut().zip(self.dmabuf.offsets()) {
            *offset = plane;
        }
        offsets
    }
}

pub(crate) fn chosen_formats(
    plane: &[Format],
    renderer: &[Format],
) -> Option<(Fourcc, Vec<Modifier>)> {
    COLOR_FORMATS.into_iter().find_map(|code| {
        let modifiers: Vec<Modifier> = renderer
            .iter()
            .filter(|format| format.code == code && format.modifier != Modifier::Invalid)
            .filter(|format| plane.contains(format))
            .map(|format| format.modifier)
            .collect();
        (!modifiers.is_empty()).then_some((code, modifiers))
    })
}

impl Imported {
    fn new(
        surface: DrmSurface,
        allocator: GbmAllocator<DrmDeviceFd>,
        renderer: &[Format],
    ) -> Result<Self, String> {
        let plane: Vec<Format> = surface.plane_info().formats.iter().copied().collect();
        let (code, modifiers) = chosen_formats(&plane, renderer).ok_or(
            "the display and the GPU share no buffer format, so the GPU cannot draw for it",
        )?;
        let (width, height) = surface.pending_mode().size();
        let slots = Slots::new(
            allocator,
            u32::from(width),
            u32::from(height),
            code,
            modifiers,
        );
        let mut imported = Self {
            surface,
            slots,
            next: None,
            pending: None,
            current: None,
        };
        imported.next_buffer()?;
        let slot = imported.next.as_ref().ok_or("no buffer was allocated")?;
        let state = imported.plane_state(slot, None);
        imported
            .surface
            .test_state([state], true)
            .map_err(|error| format!("the display does not take the GPU's buffers: {error}"))?;
        Ok(imported)
    }

    fn use_mode(&mut self, mode: Mode) -> Result<(), String> {
        self.surface
            .use_mode(mode)
            .map_err(|error| error.to_string())?;
        let (width, height) = mode.size();
        self.slots.resize(u32::from(width), u32::from(height));
        self.next = None;
        Ok(())
    }

    fn next_buffer(&mut self) -> Result<Dmabuf, String> {
        if self.next.is_none() {
            let slot = self
                .slots
                .acquire()
                .map_err(|error| format!("the GPU could not allocate a buffer: {error}"))?
                .ok_or("every buffer is still in use")?;
            if slot.userdata().get::<Framebuffer>().is_none() {
                let framebuffer = self.import(&slot)?;
                slot.userdata().insert_if_missing(|| framebuffer);
            }
            self.next = Some(slot);
        }
        let slot = self.next.as_ref().ok_or("no buffer is ready")?;
        slot.export().map_err(|error| error.to_string())
    }

    fn import(&self, slot: &Slot<GbmBuffer>) -> Result<Framebuffer, String> {
        let dmabuf = slot.export().map_err(|error| error.to_string())?;
        let drm = self.surface.device_fd().clone();
        let mut handles = Vec::new();
        let mut planes = [None; 4];
        for (plane, fd) in planes.iter_mut().zip(dmabuf.handles()) {
            let handle = match drm.prime_fd_to_buffer(fd) {
                Ok(handle) => handle,
                Err(error) => {
                    close(&drm, &handles);
                    return Err(format!("the display could not import a buffer: {error}"));
                }
            };
            if !handles.contains(&handle) {
                handles.push(handle);
            }
            *plane = Some(handle);
        }
        let planes = Planes {
            dmabuf: &dmabuf,
            handles: planes,
        };
        let added = match planes.modifier() {
            Some(modifier) => drm
                .add_planar_framebuffer(&planes, FbCmd2Flags::MODIFIERS)
                .or_else(|error| match modifier {
                    Modifier::Linear => {
                        drm.add_planar_framebuffer(&Implicit(&planes), FbCmd2Flags::empty())
                    }
                    _ => Err(error),
                }),
            None => drm.add_planar_framebuffer(&planes, FbCmd2Flags::empty()),
        };
        match added {
            Ok(fb) => Ok(Framebuffer { drm, fb, handles }),
            Err(error) => {
                close(&drm, &handles);
                Err(format!(
                    "the display could not show the GPU's buffer: {error}"
                ))
            }
        }
    }

    fn plane_state<'a>(
        &self,
        slot: &'a Slot<GbmBuffer>,
        damage: Option<&'a PlaneDamageClips>,
    ) -> PlaneState<'a> {
        let mode = self.surface.pending_mode();
        let size = (i32::from(mode.size().0), i32::from(mode.size().1));
        let fb = slot
            .userdata()
            .get::<Framebuffer>()
            .map(|framebuffer| framebuffer.fb)
            .expect("every slot is imported when it is acquired");
        PlaneState {
            handle: self.surface.plane(),
            config: Some(PlaneConfig {
                src: Rectangle::from_size(size.into()).to_f64(),
                dst: Rectangle::from_size(size.into()),
                transform: Transform::Normal,
                alpha: 1.0,
                damage_clips: damage.map(PlaneDamageClips::blob),
                fb,
                fence: None,
            }),
        }
    }

    fn queue_buffer(&mut self, sync: Option<SyncPoint>, damage: Damage) -> Result<(), String> {
        if let Some(sync) = sync {
            let _ = sync.wait();
        }
        let slot = self.next.take().ok_or("no buffer was drawn")?;
        self.slots.submitted(&slot);
        let mode = self.surface.pending_mode();
        let size = (i32::from(mode.size().0), i32::from(mode.size().1));
        let clips = damage.and_then(|damage| {
            PlaneDamageClips::from_damage(
                self.surface.device_fd(),
                Rectangle::from_size(size.into()).to_f64(),
                Rectangle::from_size(size.into()),
                damage,
            )
            .ok()
            .flatten()
        });
        let state = self.plane_state(&slot, clips.as_ref());
        let shown = match self.surface.commit_pending() {
            true => self.surface.commit([state], true),
            false => self.surface.page_flip([state], true),
        };
        shown.map_err(|error| error.to_string())?;
        self.pending = Some(slot);
        Ok(())
    }

    fn frame_submitted(&mut self) {
        if let Some(shown) = self.pending.take() {
            self.current = Some(shown);
        }
    }
}

struct Implicit<'a, 'b>(&'a Planes<'b>);

impl PlanarBuffer for Implicit<'_, '_> {
    fn size(&self) -> (u32, u32) {
        self.0.size()
    }

    fn format(&self) -> DrmFourcc {
        self.0.format()
    }

    fn modifier(&self) -> Option<DrmModifier> {
        None
    }

    fn pitches(&self) -> [u32; 4] {
        self.0.pitches()
    }

    fn handles(&self) -> [Option<buffer::Handle>; 4] {
        self.0.handles()
    }

    fn offsets(&self) -> [u32; 4] {
        self.0.offsets()
    }
}

#[cfg(test)]
mod tests;
