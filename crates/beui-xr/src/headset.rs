use std::collections::HashMap;
use std::error::Error;
use std::ffi::{CStr, CString};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use ash::vk::{self, Handle};
use beui_renderer_wgpu::present::OpenDevice;
use glam::{Quat, Vec3};
use openxr as xr;
use wgpu::hal::api::Vulkan as Api;

use crate::controllers::Controllers;
use crate::gpu::Vulkan;
use crate::runtime::{self, Loaded};
use crate::{Fill, Fov, Frame, Layers, MOST_LAYERS, Quad, Target, View};
use beui_core::window::WindowId;

const VIEWS: xr::ViewConfigurationType = xr::ViewConfigurationType::PRIMARY_STEREO;

#[derive(Clone)]
pub struct Xr(pub(crate) Arc<Shared>);

pub(crate) struct Shared {
    state: Mutex<State>,
    pub instance: xr::Instance,
    pub system: xr::SystemId,
    pub application: CString,
    blend: xr::EnvironmentBlendMode,
    runtime: String,
    _loaded: Loaded,
}

#[derive(Default)]
pub(crate) struct State {
    pub vulkan: Option<Vulkan>,
    pub device: Option<Device>,
    pub session: Option<Session>,
}

#[derive(Clone, Copy)]
pub(crate) struct Device {
    pub raw: vk::Device,
    pub family: u32,
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
        let application = application_name(application);
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
        wanted.khr_composition_layer_cylinder = available.khr_composition_layer_cylinder;
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
                    application_name: &application,
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
            application: CString::new(application).unwrap_or_default(),
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
            tried: None,
        }
    }
}

fn application_name(name: &str) -> String {
    let mut name: String = name.chars().filter(|c| *c != '\0').collect();
    while name.len() >= xr::sys::MAX_APPLICATION_NAME_SIZE {
        name.pop();
    }
    match name.is_empty() {
        true => "beui".to_owned(),
        false => name,
    }
}

pub(crate) struct Session {
    session: xr::Session<xr::Vulkan>,
    waiter: xr::FrameWaiter,
    stream: xr::FrameStream<xr::Vulkan>,
    space: xr::Space,
    controllers: Option<Controllers>,
    system: xr::SystemId,
    running: bool,
    lost: bool,
    returns: bool,
    waited: Option<xr::FrameState>,
    begun: Option<Begun>,
    swapchains: Vec<Swapchain>,
    shown: u64,
    formats: Vec<u32>,
    allowance: Layers,
    chains: HashMap<WindowId, Chain>,
    retired: Vec<(u64, Chain)>,
    layered: usize,
}

struct Chain {
    swapchain: Swapchain,
    version: Option<u64>,
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
        system: xr::SystemId,
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
        let formats = session.enumerate_swapchain_formats()?;
        let properties = instance.system_properties(system)?;
        let graphics = properties.graphics_properties;
        let allowance = Layers {
            most: (graphics.max_layer_count as usize)
                .saturating_sub(1)
                .min(MOST_LAYERS),
            cylinders: instance.exts().khr_composition_layer_cylinder.is_some(),
            largest: (
                graphics.max_swapchain_image_width,
                graphics.max_swapchain_image_height,
            ),
        };
        let controllers = Controllers::new(instance, &session)
            .inspect_err(|error| eprintln!("beui-xr: no controllers: {error}"))
            .ok();
        Ok(Self {
            controllers,
            session,
            waiter,
            stream,
            space,
            system,
            running: false,
            lost: false,
            returns: false,
            waited: None,
            begun: None,
            swapchains: Vec::new(),
            shown: 0,
            formats,
            allowance,
            chains: HashMap::new(),
            retired: Vec::new(),
            layered: 0,
        })
    }

    fn retire(&mut self, window: WindowId) {
        if let Some(chain) = self.chains.remove(&window) {
            self.retired.push((self.shown, chain));
        }
    }

    fn lose(&mut self, returns: bool) {
        self.running = false;
        self.lost = true;
        self.returns = returns;
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
const FLAT: f32 = 1e-4;
const RETIRED_FRAMES: u64 = 3;

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
    tried: Option<Instant>,
}

pub const RETRY: Duration = Duration::from_secs(2);

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

    pub fn layered(&self) -> usize {
        self.xr
            .0
            .state()
            .session
            .as_ref()
            .map_or(0, |session| session.layered)
    }

    pub fn lost(&self) -> bool {
        self.xr
            .0
            .state()
            .session
            .as_ref()
            .is_some_and(|session| session.lost)
    }

    pub fn returns(&self) -> bool {
        self.xr
            .0
            .state()
            .session
            .as_ref()
            .is_some_and(|session| session.lost && session.returns)
    }

    pub fn reopen(&mut self, device: &wgpu::Device) -> bool {
        if !self.returns() || self.tried.is_some_and(|tried| tried.elapsed() < RETRY) {
            return false;
        }
        self.tried = Some(Instant::now());
        let xr = self.xr.clone();
        let shared = &xr.0;
        let Ok(system) = shared.instance.system(xr::FormFactor::HEAD_MOUNTED_DISPLAY) else {
            return false;
        };
        let _ = device.poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: None,
        });
        let mut state = shared.state();
        state.session = None;
        let (Some(vulkan), Some(made)) = (&state.vulkan, state.device) else {
            return false;
        };
        let (vk_instance, physical) = (vulkan.instance, vulkan.physical);
        let opened = reopen(shared, system, vk_instance, physical, made);
        match opened {
            Ok(session) => {
                state.session = Some(session);
                self.anchor = None;
                self.display = None;
                true
            }
            Err(error) => {
                eprintln!("beui-xr: the headset is back but its session did not start: {error}");
                false
            }
        }
    }

    fn poll(&mut self, session: &mut Session) {
        let instance = &self.xr.0.instance;
        loop {
            let event = match instance.poll_event(&mut self.events) {
                Ok(Some(event)) => event,
                Ok(None) => break,
                Err(error) => {
                    eprintln!("beui-xr: OpenXR events stopped: {error}");
                    session.lose(false);
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
                    xr::SessionState::EXITING => session.lose(false),
                    xr::SessionState::LOSS_PENDING => session.lose(true),
                    _ => {}
                },
                xr::Event::InstanceLossPending(_) => session.lose(false),
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
                    session.lose(error != xr::sys::Result::ERROR_INSTANCE_LOST);
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
        if views.len() != session.swapchains.len() && !session.swapchains.is_empty() {
            let _ = session
                .stream
                .end(waited.predicted_display_time, xr.0.blend, &[]);
            return None;
        }
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
            views: seen(&views, &session.swapchains),
            format: session.swapchains[0].format,
            controllers: match &mut session.controllers {
                Some(controllers) => controllers.read(
                    &session.session,
                    &session.space,
                    waited.predicted_display_time,
                ),
                None => Vec::new(),
            },
            layers: session.allowance,
        };
        session.begun = Some(Begun {
            state: waited,
            views,
        });
        Some(frame)
    }

    pub fn relocate(&mut self) -> Option<Vec<View>> {
        let xr = self.xr.clone();
        let mut state = xr.0.state();
        let session = state.session.as_mut()?;
        let begun = session.begun.as_mut()?;
        let (flags, views) = session
            .session
            .locate_views(VIEWS, begun.state.predicted_display_time, &session.space)
            .ok()?;
        if !flags.contains(xr::ViewStateFlags::ORIENTATION_VALID)
            || views.len() != begun.views.len()
        {
            return None;
        }
        begun.views = views;
        Some(seen(&begun.views, &session.swapchains))
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
                session.lose(false);
                false
            }
        }
    }

    pub fn submit(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        mut commands: wgpu::CommandEncoder,
        quads: &[Quad],
        draw: &mut dyn FnMut(&[Target<'_>], &[Fill<'_>], &mut wgpu::CommandEncoder),
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
            match acquire(&mut swapchain.handle) {
                Some(image) => images.push(image),
                None => break,
            }
        }
        let complete = images.len() == session.swapchains.len();
        let (shown, filled) = match complete {
            true => layers(session, device, quads),
            false => (Vec::new(), Vec::new()),
        };
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
            let fills: Vec<Fill<'_>> = shown
                .iter()
                .map(|&quad| Fill {
                    quad,
                    texture: filled
                        .iter()
                        .find(|(filled, _)| *filled == quad)
                        .map(|&(_, image)| {
                            &session.chains[&quads[quad].window].swapchain.textures[image]
                        }),
                })
                .collect();
            draw(&targets, &fills, &mut commands);
        }
        queue.submit([commands.finish()]);
        for swapchain in session.swapchains.iter_mut().take(images.len()) {
            let _ = swapchain.handle.release_image();
        }
        for &(quad, _) in &filled {
            if let Some(chain) = session.chains.get_mut(&quads[quad].window) {
                let _ = chain.swapchain.handle.release_image();
                chain.version = Some(quads[quad].version);
            }
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
                            .image_rect(whole(swapchain.size)),
                    )
            })
            .collect();
        let mut flats = Vec::new();
        let mut cylinders = Vec::new();
        let mut order = Vec::new();
        for &index in &shown {
            let quad = &quads[index];
            let Some(chain) = session.chains.get(&quad.window) else {
                continue;
            };
            let image = xr::SwapchainSubImage::new()
                .swapchain(&chain.swapchain.handle)
                .image_rect(whole(chain.swapchain.size));
            let radius = quad.metres.0 / quad.bend;
            match quad.bend > FLAT && radius.is_finite() {
                false => {
                    order.push((false, flats.len()));
                    flats.push(
                        xr::CompositionLayerQuad::new()
                            .space(&session.space)
                            .eye_visibility(xr::EyeVisibility::BOTH)
                            .sub_image(image)
                            .pose(posef(quad.position, quad.orientation))
                            .size(xr::Extent2Df {
                                width: quad.metres.0,
                                height: quad.metres.1,
                            }),
                    );
                }
                true => {
                    let axis = quad.position + quad.orientation * Vec3::new(0.0, 0.0, radius);
                    order.push((true, cylinders.len()));
                    cylinders.push(
                        xr::CompositionLayerCylinderKHR::new()
                            .space(&session.space)
                            .eye_visibility(xr::EyeVisibility::BOTH)
                            .sub_image(image)
                            .pose(posef(axis, quad.orientation))
                            .radius(radius)
                            .central_angle(quad.bend)
                            .aspect_ratio(quad.metres.0 / quad.metres.1),
                    );
                }
            }
        }
        let flags = match order.is_empty() {
            true => xr::CompositionLayerFlags::EMPTY,
            false => xr::CompositionLayerFlags::BLEND_TEXTURE_SOURCE_ALPHA,
        };
        let projection = xr::CompositionLayerProjection::new()
            .layer_flags(flags)
            .space(&session.space)
            .views(&views);
        let mut layers: Vec<&xr::CompositionLayerBase<'_, xr::Vulkan>> = order
            .iter()
            .map(|&(cylinder, at)| match cylinder {
                false => &*flats[at],
                true => &*cylinders[at],
            })
            .collect();
        layers.push(&projection);
        match session.stream.end(time, xr.0.blend, &layers) {
            Ok(()) => {
                session.shown += 1;
                session.layered = order.len();
            }
            Err(error) => eprintln!("beui-xr: the frame was not shown: {error}"),
        }
    }

}

type Chosen = (Vec<usize>, Vec<(usize, usize)>);

fn layers(session: &mut Session, device: &wgpu::Device, quads: &[Quad]) -> Chosen {
    let frames = session.shown;
    session
        .retired
        .retain(|(since, _)| frames < since + RETIRED_FRAMES);
    let gone: Vec<WindowId> = session
        .chains
        .keys()
        .filter(|window| !quads.iter().any(|quad| quad.window == **window))
        .copied()
        .collect();
    for window in gone {
        session.retire(window);
    }
        let mut shown = Vec::new();
        let mut filled = Vec::new();
        for (index, quad) in quads.iter().enumerate() {
            let Some(format) = layer_format(&session.formats, quad.format) else {
                continue;
            };
            let fits = session.chains.get(&quad.window).is_some_and(|chain| {
                chain.swapchain.size == quad.pixels && chain.swapchain.format == format.1
            });
            if !fits {
                session.retire(quad.window);
                match swapchain(session, device, quad.pixels, format) {
                    Ok(swapchain) => {
                        session.chains.insert(
                            quad.window,
                            Chain {
                                swapchain,
                                version: None,
                            },
                        );
                    }
                    Err(error) => {
                        eprintln!("beui-xr: a panel's layer was not made: {error}");
                        continue;
                    }
                }
            }
            let Some(chain) = session.chains.get_mut(&quad.window) else {
                continue;
            };
            if chain.version == Some(quad.version) {
                shown.push(index);
                continue;
            }
            if let Some(image) = acquire(&mut chain.swapchain.handle) {
                filled.push((index, image));
                shown.push(index);
            }
        }
        (shown, filled)
}

fn acquire(handle: &mut xr::Swapchain<xr::Vulkan>) -> Option<usize> {
    let image = match handle.acquire_image() {
        Ok(image) => image as usize,
        Err(error) => {
            eprintln!("beui-xr: no swapchain image: {error}");
            return None;
        }
    };
    if let Err(error) = handle.wait_image(xr::Duration::INFINITE) {
        eprintln!("beui-xr: the swapchain image never came: {error}");
        let _ = handle.release_image();
        return None;
    }
    Some(image)
}

fn seen(views: &[xr::View], swapchains: &[Swapchain]) -> Vec<View> {
    views
        .iter()
        .zip(swapchains)
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
        .collect()
}

// unsafe(ffi)
fn reopen(
    shared: &Shared,
    system: xr::SystemId,
    vk_instance: vk::Instance,
    physical: vk::PhysicalDevice,
    device: Device,
) -> Result<Session, Box<dyn Error>> {
    shared
        .instance
        .graphics_requirements::<xr::Vulkan>(system)?;
    let picked = unsafe {
        shared
            .instance
            .vulkan_graphics_device(system, vk_instance.as_raw() as _)
    }?;
    if vk::PhysicalDevice::from_raw(picked as u64) != physical {
        return Err("it wants another GPU".into());
    }
    let (session, waiter, stream) = unsafe {
        shared.instance.create_session::<xr::Vulkan>(
            system,
            &xr::vulkan::SessionCreateInfo {
                instance: vk_instance.as_raw() as _,
                physical_device: physical.as_raw() as _,
                device: device.raw.as_raw() as _,
                queue_family_index: device.family,
                queue_index: 0,
            },
        )
    }?;
    Session::new(&shared.instance, system, session, waiter, stream)
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

fn swapchain(
    session: &Session,
    device: &wgpu::Device,
    size: (u32, u32),
    (vk_format, format): (vk::Format, wgpu::TextureFormat),
) -> Result<Swapchain, Box<dyn Error>> {
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
    let textures = handle
        .enumerate_images()?
        .into_iter()
        .map(|image| import(device, image, size, format))
        .collect::<Result<Vec<_>, Box<dyn Error>>>()?;
    Ok(Swapchain {
        handle,
        size,
        format,
        textures,
    })
}

// unsafe(ffi)
fn import(
    device: &wgpu::Device,
    image: u64,
    size: (u32, u32),
    format: wgpu::TextureFormat,
) -> Result<wgpu::Texture, Box<dyn Error>> {
    let extent = wgpu::Extent3d {
        width: size.0,
        height: size.1,
        depth_or_array_layers: 1,
    };
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
}

fn create_swapchains(
    shared: &Shared,
    session: &Session,
    device: &wgpu::Device,
) -> Result<Vec<Swapchain>, Box<dyn Error>> {
    let format = FORMATS
        .into_iter()
        .find(|(vk_format, _)| session.formats.contains(&(vk_format.as_raw() as u32)))
        .ok_or("the OpenXR runtime offers no 8-bit RGBA swapchain")?;
    shared
        .instance
        .enumerate_view_configuration_views(session.system, VIEWS)?
        .into_iter()
        .map(|view| {
            let size = (
                view.recommended_image_rect_width.max(1),
                view.recommended_image_rect_height.max(1),
            );
            swapchain(session, device, size, format)
        })
        .collect()
}

fn layer_format(
    offered: &[u32],
    content: wgpu::TextureFormat,
) -> Option<(vk::Format, wgpu::TextureFormat)> {
    let offered = |(vk_format, _): &(vk::Format, wgpu::TextureFormat)| {
        offered.contains(&(vk_format.as_raw() as u32))
    };
    FORMATS
        .into_iter()
        .filter(offered)
        .find(|(_, format)| *format == content)
        .or_else(|| {
            FORMATS
                .into_iter()
                .filter(offered)
                .find(|(_, format)| format.is_srgb() == content.is_srgb())
        })
}

fn posef(position: Vec3, orientation: Quat) -> xr::Posef {
    xr::Posef {
        orientation: xr::Quaternionf {
            x: orientation.x,
            y: orientation.y,
            z: orientation.z,
            w: orientation.w,
        },
        position: xr::Vector3f {
            x: position.x,
            y: position.y,
            z: position.z,
        },
    }
}

fn whole(size: (u32, u32)) -> xr::Rect2Di {
    xr::Rect2Di {
        offset: xr::Offset2Di { x: 0, y: 0 },
        extent: xr::Extent2Di {
            width: size.0 as i32,
            height: size.1 as i32,
        },
    }
}
