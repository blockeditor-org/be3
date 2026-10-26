use std::error::Error;
use std::ffi::CStr;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use ash::vk::{self, Handle};
use beui_renderer_wgpu::present::OpenDevice;
use glam::{Quat, Vec3};
use openxr as xr;
use wgpu::hal::api::Vulkan as Api;

use crate::gpu::Vulkan;
use crate::runtime::{self, Loaded};
use crate::{Fov, Frame, Target, View};

const VIEWS: xr::ViewConfigurationType = xr::ViewConfigurationType::PRIMARY_STEREO;

#[derive(Clone)]
pub struct Xr(pub(crate) Arc<Shared>);

pub(crate) struct Shared {
    state: Mutex<State>,
    pub instance: xr::Instance,
    pub system: xr::SystemId,
    blend: xr::EnvironmentBlendMode,
    runtime: String,
    _loaded: Loaded,
}

#[derive(Default)]
pub(crate) struct State {
    pub vulkan: Option<Vulkan>,
    pub session: Option<Session>,
}

impl Shared {
    pub fn state(&self) -> MutexGuard<'_, State> {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

impl Xr {
    pub fn open(application: &str) -> Result<Self, String> {
        let loaded = runtime::load()?;
        let available = loaded
            .entry
            .enumerate_extensions()
            .map_err(|error| format!("the OpenXR runtime lists no extensions: {error}"))?;
        if !available.khr_vulkan_enable2 {
            return Err("the OpenXR runtime does not support XR_KHR_vulkan_enable2".to_owned());
        }
        let mut wanted = xr::ExtensionSet::default();
        wanted.khr_vulkan_enable2 = true;
        wanted.ext_local_floor = available.ext_local_floor;
        #[cfg(windows)]
        {
            wanted.khr_win32_convert_performance_counter_time =
                available.khr_win32_convert_performance_counter_time;
        }
        #[cfg(not(windows))]
        {
            wanted.khr_convert_timespec_time = available.khr_convert_timespec_time;
        }
        let instance = loaded
            .entry
            .create_instance(
                &xr::ApplicationInfo {
                    application_name: application,
                    application_version: 0,
                    engine_name: "beui",
                    engine_version: 0,
                    api_version: xr::Version::new(1, 0, 0),
                },
                &wanted,
                &[],
                &(),
            )
            .map_err(|error| format!("the OpenXR instance did not open: {error}"))?;
        let runtime = instance.properties().map_or_else(
            |_| "an OpenXR runtime".to_owned(),
            |properties| format!("{} {}", properties.runtime_name, properties.runtime_version),
        );
        let system = instance
            .system(xr::FormFactor::HEAD_MOUNTED_DISPLAY)
            .map_err(|error| format!("{runtime} has no headset: {error}"))?;
        let blend = instance
            .enumerate_environment_blend_modes(system, VIEWS)
            .ok()
            .and_then(|modes| {
                modes
                    .iter()
                    .copied()
                    .find(|mode| *mode == xr::EnvironmentBlendMode::OPAQUE)
                    .or_else(|| modes.first().copied())
            })
            .unwrap_or(xr::EnvironmentBlendMode::OPAQUE);
        Ok(Self(Arc::new(Shared {
            state: Mutex::default(),
            instance,
            system,
            blend,
            runtime,
            _loaded: loaded,
        })))
    }

    pub fn runtime(&self) -> &str {
        &self.0.runtime
    }

    pub fn open_device(&self, extensions: &'static [&'static CStr]) -> OpenDevice {
        OpenDevice {
            extensions,
            gpu: Some(Arc::new(self.clone())),
        }
    }

    pub fn headset(&self) -> Headset {
        Headset {
            xr: self.clone(),
            events: xr::EventDataBuffer::new(),
            anchor: None,
            display: None,
        }
    }
}

pub(crate) struct Session {
    session: xr::Session<xr::Vulkan>,
    waiter: xr::FrameWaiter,
    stream: xr::FrameStream<xr::Vulkan>,
    space: xr::Space,
    running: bool,
    lost: bool,
    waited: Option<xr::FrameState>,
    begun: Option<Begun>,
    swapchains: Vec<Swapchain>,
    shown: u64,
}

struct Begun {
    state: xr::FrameState,
    views: Vec<xr::View>,
}

struct Swapchain {
    handle: xr::Swapchain<xr::Vulkan>,
    size: (u32, u32),
    format: wgpu::TextureFormat,
    textures: Vec<wgpu::Texture>,
}

impl Session {
    pub fn new(
        instance: &xr::Instance,
        session: xr::Session<xr::Vulkan>,
        waiter: xr::FrameWaiter,
        stream: xr::FrameStream<xr::Vulkan>,
    ) -> Result<Self, Box<dyn Error>> {
        let spaces = session.enumerate_reference_spaces()?;
        let floor = instance.exts().ext_local_floor.is_some()
            && spaces.contains(&xr::ReferenceSpaceType::LOCAL_FLOOR_EXT);
        let space = match () {
            () if floor => session.create_reference_space(
                xr::ReferenceSpaceType::LOCAL_FLOOR_EXT,
                xr::Posef::IDENTITY,
            )?,
            () if spaces.contains(&xr::ReferenceSpaceType::STAGE) => session
                .create_reference_space(xr::ReferenceSpaceType::STAGE, xr::Posef::IDENTITY)?,
            () => session.create_reference_space(
                xr::ReferenceSpaceType::LOCAL,
                xr::Posef {
                    orientation: xr::Quaternionf::IDENTITY,
                    position: xr::Vector3f {
                        x: 0.0,
                        y: -STANDING_EYES,
                        z: 0.0,
                    },
                },
            )?,
        };
        Ok(Self {
            session,
            waiter,
            stream,
            space,
            running: false,
            lost: false,
            waited: None,
            begun: None,
            swapchains: Vec::new(),
            shown: 0,
        })
    }

    fn end_unsubmitted(&mut self, blend: xr::EnvironmentBlendMode) {
        if let Some(begun) = self.begun.take() {
            let _ = self
                .stream
                .end(begun.state.predicted_display_time, blend, &[]);
        }
    }
}

const STANDING_EYES: f32 = 1.6;

impl Drop for Headset {
    fn drop(&mut self) {
        let session = self.xr.0.state().session.take();
        drop(session);
    }
}

pub struct Headset {
    xr: Xr,
    events: xr::EventDataBuffer,
    anchor: Option<(xr::Time, Instant)>,
    display: Option<Instant>,
}

impl Headset {
    pub fn runtime(&self) -> &str {
        self.xr.runtime()
    }

    pub fn running(&self) -> bool {
        self.xr
            .0
            .state()
            .session
            .as_ref()
            .is_some_and(|session| session.running)
    }

    pub fn shown(&self) -> u64 {
        self.xr
            .0
            .state()
            .session
            .as_ref()
            .map_or(0, |session| session.shown)
    }

    pub fn lost(&self) -> bool {
        self.xr
            .0
            .state()
            .session
            .as_ref()
            .is_some_and(|session| session.lost)
    }

    fn poll(&mut self, session: &mut Session) {
        let instance = &self.xr.0.instance;
        loop {
            let event = match instance.poll_event(&mut self.events) {
                Ok(Some(event)) => event,
                Ok(None) => break,
                Err(error) => {
                    eprintln!("beui-xr: OpenXR events stopped: {error}");
                    session.lost = true;
                    break;
                }
            };
            match event {
                xr::Event::SessionStateChanged(changed) => match changed.state() {
                    xr::SessionState::READY => match session.session.begin(VIEWS) {
                        Ok(_) => session.running = true,
                        Err(error) => eprintln!("beui-xr: the session did not begin: {error}"),
                    },
                    xr::SessionState::STOPPING => {
                        let _ = session.session.end();
                        session.running = false;
                        session.waited = None;
                    }
                    xr::SessionState::EXITING | xr::SessionState::LOSS_PENDING => {
                        session.running = false;
                        session.lost = true;
                    }
                    _ => {}
                },
                xr::Event::InstanceLossPending(_) => {
                    session.running = false;
                    session.lost = true;
                }
                _ => {}
            }
        }
    }

    pub fn pace(&mut self) -> Option<Instant> {
        let xr = self.xr.clone();
        let mut state = xr.0.state();
        let session = state.session.as_mut()?;
        session.end_unsubmitted(xr.0.blend);
        self.poll(session);
        if !session.running || session.lost {
            return None;
        }
        let frame = match session.waited {
            Some(frame) => frame,
            None => match session.waiter.wait() {
                Ok(frame) => frame,
                Err(error) => {
                    eprintln!("beui-xr: waiting for the frame failed: {error}");
                    session.lost = true;
                    return None;
                }
            },
        };
        if session.waited.replace(frame).is_none() || self.display.is_none() {
            self.display =
                Some(self.instant(frame.predicted_display_time, frame.predicted_display_period));
        }
        self.display
    }

    fn instant(&mut self, time: xr::Time, period: xr::Duration) -> Instant {
        let instance = &self.xr.0.instance;
        #[cfg(windows)]
        let convertible = instance
            .exts()
            .khr_win32_convert_performance_counter_time
            .is_some();
        #[cfg(not(windows))]
        let convertible = instance.exts().khr_convert_timespec_time.is_some();
        let (base, at) = match convertible.then(|| instance.now()) {
            Some(Ok(now)) => (now, Instant::now()),
            _ => *self.anchor.get_or_insert_with(|| {
                (
                    time,
                    Instant::now() + Duration::from_nanos(period.as_nanos().max(0) as u64),
                )
            }),
        };
        let ahead = time.as_nanos() - base.as_nanos();
        match ahead >= 0 {
            true => at + Duration::from_nanos(ahead as u64),
            false => at
                .checked_sub(Duration::from_nanos(ahead.unsigned_abs()))
                .unwrap_or(at),
        }
    }

    pub fn begin(&mut self, device: &wgpu::Device) -> Option<Frame> {
        let xr = self.xr.clone();
        let mut state = xr.0.state();
        let session = state.session.as_mut()?;
        let waited = session.waited.take()?;
        if let Err(error) = session.stream.begin() {
            eprintln!("beui-xr: the frame did not begin: {error}");
            return None;
        }
        let views = match waited.should_render {
            true => session
                .session
                .locate_views(VIEWS, waited.predicted_display_time, &session.space)
                .map(|(_, views)| views)
                .unwrap_or_default(),
            false => Vec::new(),
        };
        if views.is_empty() || !self.swapchains(session, device) {
            let _ = session
                .stream
                .end(waited.predicted_display_time, xr.0.blend, &[]);
            return None;
        }
        let display = match self.display.take() {
            Some(display) => display,
            None => self.instant(
                waited.predicted_display_time,
                waited.predicted_display_period,
            ),
        };
        let frame = Frame {
            display,
            views: views
                .iter()
                .zip(&session.swapchains)
                .map(|(view, swapchain)| View {
                    position: Vec3::new(
                        view.pose.position.x,
                        view.pose.position.y,
                        view.pose.position.z,
                    ),
                    orientation: Quat::from_xyzw(
                        view.pose.orientation.x,
                        view.pose.orientation.y,
                        view.pose.orientation.z,
                        view.pose.orientation.w,
                    )
                    .normalize(),
                    fov: Fov {
                        left: view.fov.angle_left,
                        right: view.fov.angle_right,
                        up: view.fov.angle_up,
                        down: view.fov.angle_down,
                    },
                    size: swapchain.size,
                })
                .collect(),
            format: session.swapchains[0].format,
        };
        session.begun = Some(Begun {
            state: waited,
            views,
        });
        Some(frame)
    }

    fn swapchains(&self, session: &mut Session, device: &wgpu::Device) -> bool {
        if !session.swapchains.is_empty() {
            return true;
        }
        match create_swapchains(&self.xr.0, session, device) {
            Ok(swapchains) => {
                session.swapchains = swapchains;
                true
            }
            Err(error) => {
                eprintln!("beui-xr: the swapchains were not made: {error}");
                session.lost = true;
                false
            }
        }
    }

    pub fn submit(
        &mut self,
        queue: &wgpu::Queue,
        mut commands: wgpu::CommandEncoder,
        draw: &mut dyn FnMut(&[Target<'_>], &mut wgpu::CommandEncoder),
    ) {
        let xr = self.xr.clone();
        let mut state = xr.0.state();
        let Some(session) = state.session.as_mut() else {
            queue.submit([commands.finish()]);
            return;
        };
        let Some(begun) = session.begun.take() else {
            queue.submit([commands.finish()]);
            return;
        };
        let mut images = Vec::new();
        for swapchain in &mut session.swapchains {
            let image = match swapchain.handle.acquire_image() {
                Ok(image) => image as usize,
                Err(error) => {
                    eprintln!("beui-xr: no swapchain image: {error}");
                    break;
                }
            };
            if let Err(error) = swapchain.handle.wait_image(xr::Duration::INFINITE) {
                eprintln!("beui-xr: the swapchain image never came: {error}");
                let _ = swapchain.handle.release_image();
                break;
            }
            images.push(image);
        }
        let complete = images.len() == session.swapchains.len();
        if complete {
            let targets: Vec<Target<'_>> = images
                .iter()
                .zip(&session.swapchains)
                .enumerate()
                .map(|(view, (&image, swapchain))| Target {
                    view,
                    texture: &swapchain.textures[image],
                })
                .collect();
            draw(&targets, &mut commands);
        }
        queue.submit([commands.finish()]);
        for swapchain in session.swapchains.iter_mut().take(images.len()) {
            let _ = swapchain.handle.release_image();
        }
        let time = begun.state.predicted_display_time;
        if !complete {
            let _ = session.stream.end(time, xr.0.blend, &[]);
            return;
        }
        let views: Vec<_> = begun
            .views
            .iter()
            .zip(&session.swapchains)
            .map(|(view, swapchain)| {
                xr::CompositionLayerProjectionView::new()
                    .pose(view.pose)
                    .fov(view.fov)
                    .sub_image(
                        xr::SwapchainSubImage::new()
                            .swapchain(&swapchain.handle)
                            .image_rect(xr::Rect2Di {
                                offset: xr::Offset2Di { x: 0, y: 0 },
                                extent: xr::Extent2Di {
                                    width: swapchain.size.0 as i32,
                                    height: swapchain.size.1 as i32,
                                },
                            }),
                    )
            })
            .collect();
        let layer = xr::CompositionLayerProjection::new()
            .space(&session.space)
            .views(&views);
        match session.stream.end(time, xr.0.blend, &[&layer]) {
            Ok(()) => session.shown += 1,
            Err(error) => eprintln!("beui-xr: the frame was not shown: {error}"),
        }
    }
}

const FORMATS: [(vk::Format, wgpu::TextureFormat); 4] = [
    (
        vk::Format::R8G8B8A8_SRGB,
        wgpu::TextureFormat::Rgba8UnormSrgb,
    ),
    (
        vk::Format::B8G8R8A8_SRGB,
        wgpu::TextureFormat::Bgra8UnormSrgb,
    ),
    (vk::Format::R8G8B8A8_UNORM, wgpu::TextureFormat::Rgba8Unorm),
    (vk::Format::B8G8R8A8_UNORM, wgpu::TextureFormat::Bgra8Unorm),
];

// unsafe(ffi)
fn create_swapchains(
    shared: &Shared,
    session: &Session,
    device: &wgpu::Device,
) -> Result<Vec<Swapchain>, Box<dyn Error>> {
    let offered = session.session.enumerate_swapchain_formats()?;
    let (vk_format, format) = FORMATS
        .into_iter()
        .find(|(vk_format, _)| offered.contains(&(vk_format.as_raw() as u32)))
        .ok_or("the OpenXR runtime offers no 8-bit RGBA swapchain")?;
    let views = shared
        .instance
        .enumerate_view_configuration_views(shared.system, VIEWS)?;
    let mut swapchains = Vec::new();
    for view in views {
        let size = (
            view.recommended_image_rect_width.max(1),
            view.recommended_image_rect_height.max(1),
        );
        let handle = session.session.create_swapchain(&xr::SwapchainCreateInfo {
            create_flags: xr::SwapchainCreateFlags::EMPTY,
            usage_flags: xr::SwapchainUsageFlags::COLOR_ATTACHMENT,
            format: vk_format.as_raw() as u32,
            sample_count: 1,
            width: size.0,
            height: size.1,
            face_count: 1,
            array_size: 1,
            mip_count: 1,
        })?;
        let extent = wgpu::Extent3d {
            width: size.0,
            height: size.1,
            depth_or_array_layers: 1,
        };
        let textures = handle
            .enumerate_images()?
            .into_iter()
            .map(|image| {
                let hal_descriptor = wgpu::hal::TextureDescriptor {
                    label: Some("xr swapchain"),
                    size: extent,
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format,
                    usage: wgpu::TextureUses::COLOR_TARGET,
                    memory_flags: wgpu::hal::MemoryFlags::empty(),
                    view_formats: Vec::new(),
                };
                let hal = unsafe { device.as_hal::<Api>() }.ok_or("the device is not Vulkan")?;
                let texture = unsafe {
                    hal.texture_from_raw(
                        vk::Image::from_raw(image),
                        &hal_descriptor,
                        Some(Box::new(|| {})),
                        wgpu::hal::vulkan::TextureMemory::External,
                    )
                };
                drop(hal);
                Ok(unsafe {
                    device.create_texture_from_hal::<Api>(
                        texture,
                        &wgpu::TextureDescriptor {
                            label: Some("xr swapchain"),
                            size: extent,
                            mip_level_count: 1,
                            sample_count: 1,
                            dimension: wgpu::TextureDimension::D2,
                            format,
                            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                            view_formats: &[],
                        },
                    )
                })
            })
            .collect::<Result<Vec<_>, Box<dyn Error>>>()?;
        swapchains.push(Swapchain {
            handle,
            size,
            format,
            textures,
        });
    }
    Ok(swapchains)
}
