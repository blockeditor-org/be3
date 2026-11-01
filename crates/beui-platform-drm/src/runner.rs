use std::cell::RefCell;
use std::error::Error;
use std::os::fd::{AsFd, BorrowedFd, OwnedFd};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use beui::{
    Keystroke, Modifiers, Pen, PenPhase, PointerButton, Pos2, ScrollDelta, TouchId, TouchPhase,
    native::Setup, native::Waker, vec2,
};
use beui_core::app::SafeArea;
use beui_core::clipboard::{PasteAnswer, PasteKind, Pasted};
use beui_core::preferences::{Motion, PreferenceLayer, ThemeKind};
use beui_core::window::{
    WindowHosting, WindowId, WindowPlacement, WindowReport, WindowRequest, WindowState,
};
use beui_runner::Incoming;
use beui_runner::renderer::Loaded;
use beui_runner::{Launch, PlatformServices};
use beui_runner::{Runner, panic_message};
use smithay::backend::allocator::gbm::GbmDevice;
use smithay::backend::drm::{
    DrmDevice, DrmDeviceFd, DrmEvent, DrmEventMetadata, DrmEventTime, DrmNode, NodeType,
};
use smithay::backend::input::Event as _;
use smithay::backend::input::{
    AbsolutePositionEvent, Axis, ButtonState, InputEvent, KeyState, KeyboardKeyEvent,
    PointerAxisEvent, PointerButtonEvent, PointerMotionEvent, ProximityState,
    TabletToolButtonEvent, TabletToolEvent, TabletToolProximityEvent, TabletToolTipEvent,
    TabletToolTipState, TabletToolType, TouchEvent,
};
use smithay::backend::libinput::{LibinputInputBackend, LibinputSessionInterface};
use smithay::backend::session::libseat::LibSeatSession;
use smithay::backend::session::{Event as SessionEvent, Session as _};
use smithay::backend::udev::{UdevBackend, UdevEvent, all_gpus, primary_gpu};
use smithay::reexports::calloop::generic::Generic;
use smithay::reexports::calloop::ping::make_ping;
use smithay::reexports::calloop::timer::{TimeoutAction, Timer};
use smithay::reexports::calloop::{
    EventLoop, Interest, LoopHandle, LoopSignal, Mode, PostAction, RegistrationToken,
};
use smithay::reexports::drm;
use smithay::reexports::drm::control::Device as ControlDevice;
use smithay::reexports::drm::control::{connector, crtc};
use smithay::reexports::input::{Device as Libdevice, DeviceCapability, Libinput};
use smithay::reexports::rustix::fs::OFlags;
use smithay::utils::{Clock, DeviceFd, Monotonic};

use be_dmabuf::{DEVICE_EXTENSIONS, Vulkan, adapter_for};

use crate::display::{DisplayConfig, DisplayControl};
use crate::displays::{Display, Displays};
use crate::epaper::{Change, Epaper, Panel};
use crate::escape::{Escape, FINGERS as ESCAPE_FINGERS, HOLD as ESCAPE_HOLD};
use crate::flips::{Flip, Flips};
use crate::gpu::{Gpu, SoftwareCursor};
use crate::input::{
    InputConfig, InputControl, InputDevice, PointerDevice, PointerSettings, PointerSpeed, RawInput,
    SharedFilter,
};
use crate::keyboard::Keyboard;
use crate::layout::{absolute, arrange, bounds, clamp, moved};
use crate::output::{Buffers, Output, connected, wait_for};
use crate::outputs::{DisplayRenderer, Outputs};
use crate::pacing::{Lane, Pacer, Wait};
use crate::problems::Problems;
use crate::recovery::{Card, Recovery};
use crate::rotation::{Rotation, Turn};
use crate::screen::FORMAT;
use crate::wake::{Held, Input, WakeGate};

const KEY_A: u32 = 30;
const LONGEST_WAIT: Duration = Duration::from_secs(3600);
const BLANKED_FRAME: Duration = Duration::from_secs(1);
const DARKEN_RETRY: Duration = Duration::from_secs(1);

pub struct Loop(pub LoopHandle<'static, Session>);

pub struct Session {
    seat: LibSeatSession,
    recovery: Recovery<connector::Handle>,
    built: u64,
    handle: LoopHandle<'static, Session>,
    signal: LoopSignal,
    drm: DrmDevice,
    buffers: Buffers,
    epaper: Option<(Panel, Epaper)>,
    escape: Escape,
    escape_timer: Option<RegistrationToken>,
    node: u64,
    libinput: Libinput,
    devices: Vec<Libdevice>,
    input: InputConfig,
    control: InputControl,
    display_control: DisplayControl,
    display_config: DisplayConfig,
    config_pending: bool,
    blanked: bool,
    gate: WakeGate,
    runner: Runner,
    platform: Seat,
    outputs: Rc<RefCell<Outputs>>,
    displays: Displays,
    windows: Windows,
    default_scale: f32,
    zoom: f32,
    pacer: Pacer,
    clock: Clock<Monotonic>,
    sampled: Option<Instant>,
    flips: Flips,
    keyboard: Keyboard,
    repeat: Option<(u32, RegistrationToken)>,
    filter_timer: Option<(Duration, RegistrationToken)>,
    wakeup: Option<(Duration, RegistrationToken)>,
    darken_retry: Option<RegistrationToken>,
    lost: Arc<AtomicBool>,
    problems: Problems,
}

#[derive(Default)]
struct Windows {
    hosted: bool,
    shown: stable_hash::HashMap<String, WindowId>,
    sized: stable_hash::HashMap<WindowId, beui::Rect>,
    scaled: stable_hash::HashMap<WindowId, f32>,
    focused: Option<WindowId>,
}

impl Windows {
    fn of(&self, display: &str) -> Option<WindowId> {
        self.shown.get(display).copied()
    }
}

struct Seat {
    clipboard: Option<String>,
    locked: bool,
    hosted: bool,
    outputs: Rc<RefCell<Outputs>>,
}

impl PlatformServices for Seat {
    fn copy(&mut self, text: String) {
        self.clipboard = Some(text);
    }

    fn paste(&mut self, kinds: &[PasteKind], answer: PasteAnswer) {
        let text = kinds
            .contains(&PasteKind::Text)
            .then(|| self.clipboard.clone())
            .flatten();
        answer.send(text.map(Pasted::Text));
    }

    fn show_window(&mut self, window: &WindowState, shown: &WindowState) {
        if self.hosted && window.requests.cursor != shown.requests.cursor {
            self.outputs.borrow_mut().icon = window.requests.cursor;
        }
        self.locked = window.requests.pointer_locked;
    }
}

pub fn run(launch: Launch) -> Result<(), Box<dyn Error>> {
    let mut event_loop: EventLoop<'static, Session> = EventLoop::try_new()?;
    let handle = event_loop.handle();
    let (mut seat, notifier) =
        LibSeatSession::new().map_err(|error| format!("no seat could be opened: {error:?}"))?;
    handle.insert_source(notifier, |event, _, session| session.seat_event(event))?;
    let name = seat.seat();
    let fd = open_display(&mut seat, &name)?;
    let fd = DrmDeviceFd::new(DeviceFd::from(fd));
    let (drm, drm_events) = DrmDevice::new(fd.clone(), true)?;
    let node = drm.device_id();
    let panel = Panel::find(&fd);
    handle.insert_source(drm_events, |event, metadata, session| match event {
        DrmEvent::VBlank(crtc) => session.flipped(crtc, metadata.as_ref()),
        DrmEvent::Error(error) => session
            .problems
            .report(format!("The display device failed: {error}")),
    })?;
    let udev = UdevBackend::new(&name)?;
    handle.insert_source(udev, |event, _, session| session.udev(event))?;
    let mut libinput = Libinput::new_with_udev(LibinputSessionInterface::from(seat.clone()));
    libinput
        .udev_assign_seat(&name)
        .map_err(|()| "the seat's input devices could not be opened")?;
    handle.insert_source(
        LibinputInputBackend::new(libinput.clone()),
        |event, _, session| session.input(event),
    )?;
    let (ping, pinged) = make_ping()?;
    handle.insert_source(pinged, |_, _, session| session.pacer.ask())?;
    let waker = Waker::new(move || ping.ping());
    let control = InputControl::new(waker.clone());
    let display_control = DisplayControl::new(waker.clone());

    let lost = Arc::new(AtomicBool::new(false));
    let (device, queue) = open_gpu(node, &lost)?;
    let gpu = Rc::new(Gpu::new(device, queue, FORMAT));
    let buffers = buffers(fd, node, &gpu)?;
    let default_scale = scale();
    let outputs = Rc::new(RefCell::new(Outputs::new(
        gpu,
        SoftwareCursor::default(),
        default_scale,
    )));
    let mut runner = Runner::new(launch);
    let hosted = runner.context().opens_windows();
    outputs.borrow_mut().hosted = hosted;
    if !hosted {
        runner.context().set_window_hosting(WindowHosting::Placed);
        runner.context().set_shows_main(false);
    }
    let displays = Displays::default();
    let mut setup = Setup::new(waker.clone());
    setup.provide(displays.clone());
    setup.provide(Loop(handle.clone()));
    setup.provide(control.clone());
    setup.provide(display_control.clone());
    let problems = Problems::default();
    setup.provide(problems.clone());
    let epaper = panel.map(|mut panel| {
        panel.take_over();
        let control = Epaper::new(waker.clone());
        setup.provide(control.clone());
        (panel, control)
    });
    let flips = outputs.borrow().flips.clone();
    setup.provide(flips.clone());
    runner.start(
        Loaded {
            renderer: Box::new(DisplayRenderer(Rc::clone(&outputs))),
            fonts: None,
        },
        setup,
    )?;
    runner.reports().fullscreen(true);
    runner.reports().scaled(default_scale);
    if epaper.is_some() {
        runner.reports().system_preferences(PreferenceLayer {
            theme: Some(ThemeKind::EPaper),
            motion: Some(Motion::Instant),
            canvas_touch: None,
        });
    }
    let input = InputConfig::default();
    let platform = Seat {
        clipboard: None,
        locked: false,
        hosted,
        outputs: Rc::clone(&outputs),
    };
    let mut session = Session {
        seat,
        recovery: Recovery::default(),
        built: 0,
        handle: handle.clone(),
        signal: event_loop.get_signal(),
        drm,
        buffers,
        epaper,
        escape: Escape::default(),
        escape_timer: None,
        node,
        libinput,
        devices: Vec::new(),
        keyboard: Keyboard::new(&input).ok_or("the keymap could not be compiled")?,
        input,
        control,
        display_control,
        display_config: DisplayConfig::default(),
        config_pending: false,
        blanked: false,
        gate: WakeGate::default(),
        runner,
        platform,
        outputs,
        displays,
        windows: Windows {
            hosted,
            ..Windows::default()
        },
        default_scale,
        zoom: 1.0,
        pacer: Pacer::default(),
        clock: Clock::new(),
        sampled: None,
        flips,
        repeat: None,
        filter_timer: None,
        wakeup: None,
        darken_retry: None,
        lost,
        problems,
    };
    if let Some(config) = session.display_control.take() {
        session.display_config = config;
    }
    session.scan();
    if session.outputs.borrow().outputs.is_empty() {
        return Err("no display is connected".into());
    }
    {
        let mut outputs = session.outputs.borrow_mut();
        outputs.pointer = outputs.outputs[0].screen.rect.center();
    }
    session.pacer.ask();
    let ran = catch_unwind(AssertUnwindSafe(|| {
        event_loop.run(None, &mut session, Session::idle)
    }));
    let finished = session.end();
    drop(event_loop);
    match ran {
        Ok(ran) => ran?,
        Err(payload) => {
            return Err(format!(
                "the session ended after a panic: {}",
                panic_message(&*payload)
            )
            .into());
        }
    }
    finished
}

fn open_display(seat: &mut LibSeatSession, name: &str) -> Result<OwnedFd, String> {
    let mut paths: Vec<PathBuf> = primary_gpu(name).ok().flatten().into_iter().collect();
    for path in all_gpus(name).unwrap_or_default() {
        if !paths.contains(&path) {
            paths.push(path);
        }
    }
    if paths.is_empty() {
        return Err("no GPU was found on this seat".to_owned());
    }
    let mut failures = Vec::new();
    for path in paths {
        let fd = match seat.open(
            &path,
            OFlags::RDWR | OFlags::CLOEXEC | OFlags::NOCTTY | OFlags::NONBLOCK,
        ) {
            Ok(fd) => fd,
            Err(error) => {
                failures.push(format!("{} could not be opened: {error:?}", path.display()));
                continue;
            }
        };
        match Probe(fd.as_fd()).resource_handles() {
            Ok(resources) if !resources.connectors().is_empty() => return Ok(fd),
            Ok(_) => failures.push(format!("{} has no connectors", path.display())),
            Err(error) => {
                failures.push(format!("{} cannot drive displays: {error}", path.display()))
            }
        }
        let _ = seat.close(fd);
    }
    Err(format!(
        "no device on this seat can drive a display: {}",
        failures.join("; ")
    ))
}

struct Probe<'a>(BorrowedFd<'a>);

impl AsFd for Probe<'_> {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.0
    }
}

impl drm::Device for Probe<'_> {}

impl ControlDevice for Probe<'_> {}

fn buffers(display: DrmDeviceFd, node: u64, gpu: &Gpu) -> Result<Buffers, Box<dyn Error>> {
    let render = gpu.vulkan().and_then(Vulkan::render_node);
    let own = DrmNode::from_dev_id(node)
        .ok()
        .and_then(|node| node.node_with_type(NodeType::Render))
        .and_then(Result::ok)
        .map(|node| node.dev_id());
    let Some(render) = render.filter(|render| own != Some(*render)) else {
        return Ok(Buffers {
            gbm: GbmDevice::new(display)?,
            imported: false,
        });
    };
    let path = DrmNode::from_dev_id(render)?
        .dev_path()
        .ok_or("the GPU's render node has no path")?;
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(&path)
        .map_err(|error| format!("{} could not be opened: {error}", path.display()))?;
    eprintln!(
        "beui: the display device cannot draw, so {} draws for it",
        path.display()
    );
    Ok(Buffers {
        gbm: GbmDevice::new(DrmDeviceFd::new(DeviceFd::from(OwnedFd::from(file))))?,
        imported: true,
    })
}

fn scale() -> f32 {
    std::env::var("BEUI_SCALE")
        .ok()
        .and_then(|scale| scale.parse::<f32>().ok())
        .filter(|scale| *scale > 0.0)
        .unwrap_or(1.0)
}

fn open_gpu(node: u64, lost: &Arc<AtomicBool>) -> Result<(wgpu::Device, wgpu::Queue), String> {
    let adapter = adapter_for(node).ok_or("no Vulkan device was found")?;
    let descriptor = wgpu::DeviceDescriptor {
        label: Some("beui device"),
        required_features: beui_host::native::browser_features(&adapter),
        required_limits: adapter.limits(),
        experimental_features: wgpu::ExperimentalFeatures::disabled(),
        memory_hints: wgpu::MemoryHints::Performance,
        trace: wgpu::Trace::Off,
    };
    let (device, queue) =
        beui_host::native::open_vulkan_device(&adapter, &descriptor, &DEVICE_EXTENSIONS)
            .ok_or("the Vulkan device could not be opened")?;
    let flag = lost.clone();
    device.set_device_lost_callback(move |_, message| {
        eprintln!("beui: the GPU was lost: {message}");
        flag.store(true, Ordering::SeqCst);
    });
    Ok((device, queue))
}

impl Session {
    fn end(self) -> Result<(), Box<dyn Error>> {
        let Session {
            runner,
            platform,
            outputs,
            devices,
            libinput,
            buffers,
            epaper,
            drm,
            ..
        } = self;
        let finished = runner.finish();
        drop(platform);
        drop(outputs);
        drop(devices);
        libinput.suspend();
        drop(libinput);
        drop(buffers);
        drop(epaper);
        drop(drm);
        finished
    }

    fn idle(&mut self) {
        if self.lost.swap(false, Ordering::SeqCst) {
            self.recover();
        }
        if let Some(config) = self.control.take() {
            self.configure(config);
        }
        self.drain_filter();
        if let Some(config) = self.display_control.take() {
            self.display_config = config;
            self.config_pending = true;
        }
        let refresh = self
            .epaper
            .as_ref()
            .is_some_and(|(_, control)| control.take_refresh());
        if !self.recovery.active() {
            return;
        }
        if refresh && let Some((panel, _)) = &self.epaper {
            panel.refresh();
        }
        if self.config_pending {
            self.apply_config();
        }
        if self.runner.has_events() {
            self.pacer.ask();
        }
        self.pace_epaper();
        if self.pacer.wait(self.now(), &self.lanes()) == Wait::Now {
            self.update();
            self.pace_epaper();
        }
        if let Some(blanked) = self.display_control.take_blanked() {
            self.set_blanked(blanked);
        }
        self.runner.present();
        if self.blanked {
            self.darken();
        }
        self.watch_fences();
        self.check_failures();
        self.schedule();
    }

    fn now(&self) -> Duration {
        self.clock.now().into()
    }

    fn pace_epaper(&mut self) {
        let now = self.now();
        let Some((panel, _)) = &mut self.epaper else {
            return;
        };
        let Some(change) = panel.pace.step(now) else {
            return;
        };
        panel.change(change);
        for output in &mut self.outputs.borrow_mut().outputs {
            match change {
                Change::Hurry => output.screen.hurry(),
                Change::Settle => output.screen.settle(),
            }
        }
        if change == Change::Settle {
            self.pacer.ask();
        }
    }

    fn lanes(&self) -> Vec<Lane> {
        self.outputs
            .borrow()
            .outputs
            .iter()
            .filter_map(Output::lane)
            .collect()
    }

    fn with_recovery(&mut self, step: impl FnOnce(&mut Recovery<connector::Handle>, &mut Self)) {
        let mut recovery = std::mem::take(&mut self.recovery);
        step(&mut recovery, self);
        self.recovery = recovery;
    }

    fn check_failures(&mut self) {
        let failures: Vec<_> = self
            .outputs
            .borrow_mut()
            .outputs
            .iter_mut()
            .filter_map(|output| Some((output.connector, output.take_failure()?)))
            .collect();
        for (connector, problem) in failures {
            self.with_recovery(|recovery, session| recovery.failed(session, connector, problem));
        }
    }

    fn apply_config(&mut self) {
        let mut changed = false;
        let mut waiting = false;
        {
            let mut outputs = self.outputs.borrow_mut();
            let outputs = &mut *outputs;
            for output in &mut outputs.outputs {
                if output.drawing() {
                    waiting = true;
                    continue;
                }
                let wanted = self.display_config.mode(output.id());
                let rotation = self.display_config.rotation(output.id());
                let scale = self
                    .display_config
                    .scale(output.id())
                    .unwrap_or(self.default_scale);
                changed |= output.configure(&outputs.gpu, wanted, rotation, scale);
            }
        }
        self.config_pending = waiting;
        if changed {
            self.relayout();
        }
    }

    fn set_blanked(&mut self, blanked: bool) {
        if blanked == self.blanked {
            return;
        }
        self.blanked = blanked;
        self.gate.set_blanked(blanked);
        for output in &mut self.outputs.borrow_mut().outputs {
            output.set_blanked(blanked);
        }
        self.pacer.ask();
    }

    fn darken(&mut self) {
        let lit = {
            let mut displays = self.outputs.borrow_mut();
            for output in &mut displays.outputs {
                output.darken();
            }
            displays.outputs.iter().any(Output::lit_while_blanked)
        };
        if lit && self.darken_retry.is_none() {
            self.darken_retry = self
                .handle
                .insert_source(Timer::from_duration(DARKEN_RETRY), |_, _, session| {
                    session.darken_retry = None;
                    session.pacer.ask();
                    TimeoutAction::Drop
                })
                .ok();
        }
    }

    fn watch_fences(&mut self) {
        let fences: Vec<_> = self
            .outputs
            .borrow_mut()
            .outputs
            .iter_mut()
            .filter_map(|output| {
                let (generation, frame, fence) = output.take_fence()?;
                Some((output.crtc, generation, frame, fence))
            })
            .collect();
        for (crtc, generation, frame, fence) in fences {
            let spare = fence.try_clone();
            let watched = self.handle.insert_source(
                Generic::new(fence, Interest::READ, Mode::OneShot),
                move |_, _, session| {
                    session.drawn(crtc, generation, frame);
                    session.check_failures();
                    Ok(PostAction::Remove)
                },
            );
            if let Err(error) = watched {
                eprintln!(
                    "beui: a frame's fence could not be watched, so it is waited for: {error}"
                );
                if let Ok(spare) = spare {
                    let _ = wait_for(&spare);
                }
                self.drawn(crtc, generation, frame);
            }
        }
    }

    fn drawn(&mut self, crtc: crtc::Handle, generation: u64, frame: u64) {
        let mut displays = self.outputs.borrow_mut();
        if let Some(output) = displays
            .outputs
            .iter_mut()
            .find(|output| output.crtc == crtc)
        {
            output.drawn(generation, frame);
        }
    }

    fn report_monitors(&self) {
        let monitors = self
            .outputs
            .borrow()
            .outputs
            .iter()
            .map(Output::monitor)
            .collect();
        self.display_control.set_monitors(monitors);
    }

    fn rects(&self) -> Vec<beui::Rect> {
        self.outputs.borrow().rects()
    }

    fn update(&mut self) {
        let now = self.now();
        for output in &mut self.outputs.borrow_mut().outputs {
            output.framed();
        }
        let Some(frame) = self.runner.frame(&mut self.platform, SafeArea::default()) else {
            self.pacer.ran(now, false, None);
            if self.runner.exited() {
                self.signal.stop();
            }
            return;
        };
        self.match_windows();
        self.follow_zoom();
        let pending = self.prepare_outputs();
        if let Some((panel, _)) = &mut self.epaper {
            panel.pace.animating(frame.animating);
            if pending {
                panel.pace.drew(now);
            }
        }
        let wait = |after: Duration| (after != Duration::MAX).then(|| after.min(LONGEST_WAIT));
        match self.blanked {
            false => self.pacer.ran(
                now,
                frame.deferred || frame.repaint,
                wait(frame.repaint_after),
            ),
            true => self.pacer.ran(
                now,
                frame.deferred,
                wait(frame.repaint_after.max(BLANKED_FRAME)),
            ),
        }
    }

    fn schedule(&mut self) {
        let now = self.now();
        let paced = match self.pacer.wait(now, &self.lanes()) {
            Wait::Idle => None,
            Wait::Now => Some(now),
            Wait::Until(at) => Some(at),
        };
        let settles = self
            .epaper
            .as_ref()
            .and_then(|(panel, _)| panel.pace.settles_at())
            .map(|at| at.max(now));
        let at = paced.into_iter().chain(settles).min();
        if self.wakeup.as_ref().map(|(held, _)| *held) == at && at != Some(now) {
            return;
        }
        if let Some((_, token)) = self.wakeup.take() {
            self.handle.remove(token);
        }
        let Some(at) = at else {
            return;
        };
        self.wakeup = self
            .handle
            .insert_source(
                Timer::from_duration(at.saturating_sub(now)),
                |_, _, session| {
                    session.wakeup = None;
                    TimeoutAction::Drop
                },
            )
            .ok()
            .map(|token| (at, token));
    }

    fn flipped(&mut self, crtc: crtc::Handle, metadata: Option<&DrmEventMetadata>) {
        let shown = self
            .outputs
            .borrow_mut()
            .outputs
            .iter_mut()
            .find(|output| output.crtc == crtc)
            .and_then(|output| {
                output
                    .flipped()
                    .then(|| (output.connector, output.id().to_owned(), output.refresh()))
            });
        if let Some((connector, screen, refresh)) = shown {
            self.recovery.shown(connector);
            let (time, hardware_clock) = match metadata.map(|metadata| metadata.time) {
                Some(DrmEventTime::Monotonic(time)) => (time, true),
                _ => (self.now(), false),
            };
            let flip = Flip {
                screen,
                time,
                refresh,
                sequence: metadata.map_or(0, |metadata| u64::from(metadata.sequence)),
                hardware_clock,
            };
            if self.flips.shown(flip) {
                self.pacer.ask();
            }
        }
        self.check_failures();
    }

    fn scan(&mut self) {
        let connectors = connected(&self.drm);
        let mut guard = self.outputs.borrow_mut();
        let displays = &mut *guard;
        displays.outputs.retain(|output| {
            connectors
                .iter()
                .any(|(handle, _)| *handle == output.connector)
        });
        let gpu = Rc::clone(&displays.gpu);
        for (handle, info) in connectors {
            if displays
                .outputs
                .iter()
                .any(|output| output.connector == handle)
            {
                continue;
            }
            let taken: Vec<_> = displays.outputs.iter().map(|output| output.crtc).collect();
            self.built += 1;
            match Output::new(
                &mut self.drm,
                &self.buffers,
                &gpu,
                handle,
                &info,
                &taken,
                &self.display_config,
                self.default_scale,
                self.built,
            ) {
                Ok(mut output) => {
                    output.set_blanked(self.blanked);
                    displays.outputs.push(output);
                }
                Err(error) => self
                    .problems
                    .report(format!("A display could not be used: {error}")),
            }
        }
        drop(guard);
        self.relayout();
    }

    fn relayout(&mut self) {
        let mut guard = self.outputs.borrow_mut();
        let outputs = &mut *guard;
        let sizes: Vec<_> = outputs
            .outputs
            .iter()
            .map(|output| (output.screen.picture(), output.screen.scale))
            .collect();
        for (output, rect) in outputs.outputs.iter_mut().zip(arrange(&sizes)) {
            output.screen.rect = rect;
            output.screen.invalidate();
        }
        outputs.pointer = clamp(outputs.pointer, &outputs.rects());
        outputs.follow_scales();
        let zoom = self.zoom;
        for output in &mut outputs.outputs {
            output.screen.zoom = zoom;
        }
        let shown: Vec<Display> = outputs
            .outputs
            .iter()
            .map(|output| Display {
                id: output.id().to_owned(),
                name: output.name().to_owned(),
                position: beui::pos2(
                    output.screen.rect.min.x / zoom,
                    output.screen.rect.min.y / zoom,
                ),
                size: output.screen.rect.size() / zoom,
                scale: output.screen.scale * zoom,
            })
            .collect();
        let native = outputs.outputs.first().map(|first| first.screen.scale);
        drop(guard);
        if let Some(native) = native {
            self.runner.reports().scaled(native);
        }
        self.displays.set(shown);
        self.report_sizes();
        self.pacer.ask();
        self.report_monitors();
    }

    fn follow_zoom(&mut self) {
        let zoom = self.runner.context().zoom_factor();
        if zoom != self.zoom {
            self.zoom = zoom;
            self.relayout();
        }
    }

    fn report_sizes(&mut self) {
        let displays = self.displays.displays();
        let mut reports = Vec::new();
        for (id, window) in &self.windows.shown {
            let Some(display) = displays.iter().find(|display| display.id == *id) else {
                continue;
            };
            let rect = display.rect();
            if self.windows.sized.insert(*window, rect) != Some(rect) {
                reports.push(WindowReport::Resized {
                    window: *window,
                    size: display.size,
                });
                reports.push(WindowReport::Moved {
                    window: *window,
                    origin: display.position,
                });
            }
            if self.windows.scaled.insert(*window, display.scale) != Some(display.scale) {
                reports.push(WindowReport::Rastered {
                    window: *window,
                    pixels_per_point: display.scale,
                });
            }
        }
        for report in reports {
            self.runner.push(Incoming::Window(report));
        }
    }

    fn match_windows(&mut self) {
        if self.windows.hosted {
            return;
        }
        let Some(output) = self.runner.output() else {
            return;
        };
        let mut opened = Vec::new();
        for request in &output.window_requests {
            match request {
                WindowRequest::Open {
                    window,
                    placement: WindowPlacement::Host(placement),
                    ..
                } => {
                    if let Some(id) = self.displays.placed(placement) {
                        opened.push(*window);
                        if let Some(old) = self.windows.shown.insert(id, *window) {
                            self.windows.sized.remove(&old);
                            self.windows.scaled.remove(&old);
                        }
                    }
                }
                WindowRequest::Close(window) => {
                    self.windows.shown.retain(|_, held| held != window);
                    self.windows.sized.remove(window);
                    self.windows.scaled.remove(window);
                    if self.windows.focused == Some(*window) {
                        self.windows.focused = None;
                    }
                }
                _ => {}
            }
        }
        if opened.is_empty() {
            return;
        }
        self.report_sizes();
        let first = self
            .outputs
            .borrow()
            .outputs
            .first()
            .and_then(|output| self.windows.of(output.id()));
        if self.windows.focused.is_none()
            && let Some(first) = first
        {
            self.focus_window(first);
        }
        self.pacer.ask();
    }

    fn focus_window(&mut self, window: WindowId) {
        if self.windows.focused == Some(window) {
            return;
        }
        if let Some(old) = self.windows.focused.replace(window) {
            self.runner.push(Incoming::Window(WindowReport::Focused {
                window: old,
                focused: false,
            }));
        }
        self.runner.push(Incoming::Window(WindowReport::Focused {
            window,
            focused: true,
        }));
    }

    fn prepare_outputs(&mut self) -> bool {
        let Some(output) = self.runner.output() else {
            return false;
        };
        let windows = &self.windows;
        let mut outputs = self.outputs.borrow_mut();
        let first = outputs.outputs.first().map(|first| first.id().to_owned());
        if !windows.hosted {
            let pointer = outputs.pointer;
            let icon = outputs
                .under(pointer)
                .and_then(|under| windows.of(under.id()))
                .and_then(|window| output.window(window))
                .map(|window| window.window.requests.cursor);
            if let Some(icon) = icon {
                outputs.icon = icon;
            }
        }
        let runner = &self.runner;
        outputs.prepare(|id| match windows.hosted {
            true => (first.as_deref() == Some(id))
                .then(|| runner.shown_window(WindowId::MAIN))
                .flatten(),
            false => output.window(windows.of(id)?),
        })
    }

    fn at(&self, position: Pos2) -> (WindowId, Pos2) {
        let unzoomed = beui::pos2(position.x / self.zoom, position.y / self.zoom);
        if self.windows.hosted {
            return (WindowId::MAIN, unzoomed);
        }
        let outputs = self.outputs.borrow();
        outputs
            .under(position)
            .and_then(|output| {
                let window = self.windows.of(output.id())?;
                let local = (position - output.screen.rect.min) / self.zoom;
                Some((window, beui::pos2(local.x, local.y)))
            })
            .unwrap_or((WindowId::MAIN, unzoomed))
    }

    fn pointed(&mut self, position: Pos2, input: impl FnOnce(Pos2) -> beui::Input) {
        let (window, local) = self.at(position);
        self.push_sample(Incoming::points(input(local)).in_window(window));
    }

    fn scroll(&mut self, delta: ScrollDelta) {
        let (window, _) = self.at(self.pointer());
        self.push_sample(Incoming::points(beui::Input::Scroll(delta)).in_window(window));
    }

    fn pressed_at(&mut self, position: Pos2) {
        let (window, _) = self.at(position);
        if !window.is_main() {
            self.focus_window(window);
        }
    }

    fn recover(&mut self) {
        eprintln!("beui: the GPU was lost, so the session ends");
        self.signal.stop();
    }

    fn seat_event(&mut self, event: SessionEvent) {
        match event {
            SessionEvent::PauseSession => {
                self.with_recovery(|recovery, session| recovery.pause(session))
            }
            SessionEvent::ActivateSession => {
                self.with_recovery(|recovery, session| recovery.activate(session));
            }
        }
    }

    fn udev(&mut self, event: UdevEvent) {
        if let UdevEvent::Changed { device_id } = event
            && device_id == self.node
        {
            self.with_recovery(|recovery, session| recovery.changed(session));
        }
    }

    fn push(&mut self, input: impl Into<beui_runner::Incoming>) {
        self.runner.push(input);
        self.pacer.ask();
    }

    fn push_sample(&mut self, event: impl Into<beui_runner::Incoming>) {
        self.push(event);
        if let Some(at) = self.sampled {
            self.push(beui_runner::Incoming::Time(at));
        }
    }

    fn point(&mut self, pointer: Pos2) {
        self.place_pointer(pointer);
        self.pointed(pointer, beui::Input::PointerMoved);
    }

    fn place_pointer(&mut self, pointer: Pos2) {
        let mut outputs = self.outputs.borrow_mut();
        outputs.pointer = pointer;
        outputs.moved_pointer();
    }

    fn gated(&mut self, event: &InputEvent<LibinputInputBackend>) -> bool {
        let Some((input, time)) = gate_input(event) else {
            return true;
        };
        let pass = self.gate.pass(input, time);
        if pass.woke {
            self.set_blanked(false);
            self.display_control.woke();
            self.pacer.ask();
        }
        if !pass.deliver {
            match event {
                InputEvent::PointerMotion { event } if !self.platform.locked => {
                    let delta = vec2(event.delta_x() as f32, event.delta_y() as f32);
                    self.place_pointer(moved(self.pointer(), delta, &self.rects()));
                }
                InputEvent::PointerMotionAbsolute { event } => {
                    let position = self.absolute_position(event);
                    self.place_pointer(position);
                }
                InputEvent::TabletToolAxis { event } => {
                    let position = self.absolute_position(event);
                    self.place_pointer(position);
                }
                InputEvent::TabletToolProximity { event } => {
                    let position = self.absolute_position(event);
                    self.place_pointer(position);
                }
                _ => {}
            }
        }
        pass.deliver
    }

    fn absolute_position<E: AbsolutePositionEvent<LibinputInputBackend>>(&self, event: &E) -> Pos2 {
        let (area, rotation) = self.absolute_area();
        clamp(absolute(normalized(event), area, rotation), &self.rects())
    }

    fn absolute_area(&self) -> (beui::Rect, Rotation) {
        let outputs = self.outputs.borrow();
        match outputs.outputs.as_slice() {
            [only] => (only.screen.rect, only.screen.rotation()),
            _ => (bounds(&outputs.rects()), Rotation::Normal),
        }
    }

    fn pointer(&self) -> Pos2 {
        self.outputs.borrow().pointer
    }

    fn configure(&mut self, mut config: InputConfig) {
        if config == self.input {
            return;
        }
        if !config.same_keymap(&self.input) {
            match Keyboard::new(&config) {
                Some(keyboard) => self.keyboard = keyboard,
                None => {
                    self.problems.report(format!(
                        "The keyboard layout {:?} ({:?}, {:?}) could not be compiled, so the last one stays.",
                        config.layout, config.variant, config.options
                    ));
                    config.layout.clone_from(&self.input.layout);
                    config.variant.clone_from(&self.input.variant);
                    config.options.clone_from(&self.input.options);
                }
            }
        }
        self.stop_repeat();
        self.input = config;
        for device in &mut self.devices {
            configure_device(device, &self.input);
        }
    }

    fn report_devices(&self) {
        let mut pointers: Vec<PointerDevice> = Vec::new();
        for device in &self.devices {
            let defaults = pointer_defaults(device);
            if defaults == PointerSettings::default() {
                continue;
            }
            let id = device_id(device);
            if pointers.iter().any(|pointer| pointer.id == id) {
                continue;
            }
            pointers.push(PointerDevice { id, defaults });
        }
        pointers.sort_by(|left, right| left.id.cmp(&right.id));
        self.control.set_devices(pointers);
        let typing = self.devices.iter().any(|device| {
            device.has_capability(DeviceCapability::Keyboard)
                && device.keyboard_has_key(KEY_A) == Ok(true)
        });
        self.runner.reports().keyboard_attached(typing);
        let touch = self
            .devices
            .iter()
            .any(|device| device.has_capability(DeviceCapability::Touch));
        self.runner.reports().touchscreen_attached(touch);
    }

    fn input(&mut self, event: InputEvent<LibinputInputBackend>) {
        if !self.gated(&event) {
            return;
        }
        let now = self.now();
        let gated = gate_input(&event);
        if let Some((panel, _)) = &mut self.epaper
            && let Some((input, _)) = gated
        {
            panel.pace.input(input, now);
        }
        self.sampled = gated.map(|(_, micros)| {
            let ago = now.saturating_sub(Duration::from_micros(micros));
            let instant = Instant::now();
            instant.checked_sub(ago).unwrap_or(instant)
        });
        self.event(event);
        self.sampled = None;
    }

    fn event(&mut self, event: InputEvent<LibinputInputBackend>) {
        match event {
            InputEvent::DeviceAdded { mut device } => {
                configure_device(&mut device, &self.input);
                self.devices.push(device);
                self.report_devices();
            }
            InputEvent::DeviceRemoved { device } => {
                self.devices.retain(|known| *known != device);
                self.report_devices();
            }
            InputEvent::Keyboard { event } => {
                let code = event.key_code().raw().saturating_sub(8);
                self.filtered(RawInput::Key {
                    code,
                    pressed: event.state() == KeyState::Pressed,
                });
            }
            InputEvent::PointerMotion { event } => {
                let delta = vec2(event.delta_x() as f32, event.delta_y() as f32);
                if self.platform.locked {
                    self.push(Incoming::points(beui::Input::PointerMotion(delta)));
                    return;
                }
                self.point(moved(self.pointer(), delta, &self.rects()));
            }
            InputEvent::PointerMotionAbsolute { event } => {
                let position = self.absolute_position(&event);
                self.point(position);
            }
            InputEvent::PointerButton { event } => {
                self.filtered(RawInput::Button {
                    code: event.button_code(),
                    pressed: event.state() == ButtonState::Pressed,
                });
            }
            InputEvent::PointerAxis { event } => {
                let notch = |axis| event.amount_v120(axis).map(|v120| v120.round() as i32);
                let (x, y) = (notch(Axis::Horizontal), notch(Axis::Vertical));
                if x.is_some() || y.is_some() {
                    self.filtered(RawInput::Wheel {
                        x: x.unwrap_or(0),
                        y: y.unwrap_or(0),
                    });
                    return;
                }
                let amount = |axis| -event.amount(axis).unwrap_or(0.0) as f32;
                let delta =
                    ScrollDelta::Pixels(vec2(amount(Axis::Horizontal), amount(Axis::Vertical)));
                self.scroll(delta);
            }
            InputEvent::TouchDown { event } => {
                self.escape_touch(event.slot(), true);
                let position = self.touch_position(&event);
                self.touch(event.slot(), TouchPhase::Start, position);
            }
            InputEvent::TouchMotion { event } => {
                let position = self.touch_position(&event);
                self.touch(event.slot(), TouchPhase::Move, position);
            }
            InputEvent::TouchUp { event } => {
                self.escape_touch(event.slot(), false);
                self.touch(event.slot(), TouchPhase::End, self.pointer());
            }
            InputEvent::TouchCancel { event } => {
                self.escape_touch(event.slot(), false);
                self.touch(event.slot(), TouchPhase::Cancel, self.pointer());
            }
            InputEvent::TabletToolProximity { event } => {
                let phase = match event.state() {
                    ProximityState::In => PenPhase::Move,
                    ProximityState::Out => PenPhase::Gone,
                };
                self.pen(phase, &event);
            }
            InputEvent::TabletToolAxis { event } => self.pen(PenPhase::Move, &event),
            InputEvent::TabletToolTip { event } => {
                let phase = match TabletToolTipEvent::tip_state(&event) {
                    TabletToolTipState::Down => PenPhase::Down(PointerButton::Primary),
                    TabletToolTipState::Up => PenPhase::Up(PointerButton::Primary),
                };
                self.pen(phase, &event);
            }
            InputEvent::TabletToolButton { event } => {
                let button = stylus_button(TabletToolButtonEvent::button(&event));
                let phase = match TabletToolButtonEvent::button_state(&event) {
                    ButtonState::Pressed => PenPhase::Down(button),
                    ButtonState::Released => PenPhase::Up(button),
                };
                self.pen(phase, &event);
            }
            _ => {}
        }
    }

    fn escape_touch(&mut self, slot: smithay::backend::input::TouchSlot, down: bool) {
        match self.escape.touch(slot.into(), down) {
            Some(true) => {
                self.escape_timer = self
                    .handle
                    .insert_source(Timer::from_duration(ESCAPE_HOLD), |_, _, session| {
                        eprintln!(
                            "beui: {ESCAPE_FINGERS} fingers were held on the screen, so the session ends"
                        );
                        session.escape_timer = None;
                        session.signal.stop();
                        TimeoutAction::Drop
                    })
                    .ok();
            }
            Some(false) => {
                if let Some(token) = self.escape_timer.take() {
                    self.handle.remove(token);
                }
            }
            None => {}
        }
    }

    fn pen<E: TabletToolEvent<LibinputInputBackend>>(&mut self, phase: PenPhase, event: &E) {
        let position = self.absolute_position(event);
        self.place_pointer(position);
        let tool = event.tool();
        let (_, rotation) = self.absolute_area();
        let tilt =
            rotation.direction_to_picture(vec2(event.tilt_x() as f32, event.tilt_y() as f32));
        if matches!(phase, PenPhase::Down(_)) {
            self.pressed_at(position);
        }
        let pen = Pen {
            pressure: event.pressure() as f32,
            tilt,
            eraser: tool.tool_type == TabletToolType::Eraser,
        };
        self.pointed(position, |pos| beui::Input::Pen { phase, pos, pen });
    }

    fn touch_position<E: AbsolutePositionEvent<LibinputInputBackend>>(&self, event: &E) -> Pos2 {
        let outputs = self.outputs.borrow();
        let (rect, rotation) = outputs
            .outputs
            .first()
            .map_or((beui::Rect::ZERO, Rotation::Normal), |output| {
                (output.screen.rect, output.screen.rotation())
            });
        absolute(normalized(event), rect, rotation)
    }

    fn touch(
        &mut self,
        slot: smithay::backend::input::TouchSlot,
        phase: TouchPhase,
        position: Pos2,
    ) {
        let finger: i32 = slot.into();
        if phase == TouchPhase::Start {
            self.pressed_at(position);
        }
        let id = TouchId {
            device: 0,
            finger: u64::from(finger.max(0).unsigned_abs()),
        };
        self.pointed(position, |pos| beui::Input::Touch {
            id,
            phase,
            pos,
            force: None,
        });
    }

    fn filtered(&mut self, input: RawInput) {
        let Some(filter) = self.control.filter() else {
            self.deliver(input);
            return;
        };
        let outputs = filter.borrow_mut().input(input, self.now());
        for output in outputs {
            self.deliver(output);
        }
        self.schedule_filter(&filter);
    }

    fn drain_filter(&mut self) {
        for input in self.control.take_injected() {
            self.deliver(input);
        }
        match self.control.filter() {
            Some(filter) => self.schedule_filter(&filter),
            None => self.stop_filter_timer(),
        }
    }

    fn fire_filter(&mut self) {
        self.filter_timer = None;
        let Some(filter) = self.control.filter() else {
            return;
        };
        let outputs = filter.borrow_mut().timer(self.now());
        for output in outputs {
            self.deliver(output);
        }
        self.schedule_filter(&filter);
    }

    fn schedule_filter(&mut self, filter: &SharedFilter) {
        let deadline = filter.borrow().deadline();
        if self.filter_timer.as_ref().map(|(at, _)| *at) == deadline {
            return;
        }
        self.stop_filter_timer();
        let Some(at) = deadline else {
            return;
        };
        let wait = at.saturating_sub(self.now());
        let token = self
            .handle
            .insert_source(Timer::from_duration(wait), |_, _, session| {
                session.fire_filter();
                TimeoutAction::Drop
            });
        if let Ok(token) = token {
            self.filter_timer = Some((at, token));
        }
    }

    fn stop_filter_timer(&mut self) {
        if let Some((_, token)) = self.filter_timer.take() {
            self.handle.remove(token);
        }
    }

    fn deliver(&mut self, input: RawInput) {
        match input {
            RawInput::Key { code, pressed } => self.key(code, pressed),
            RawInput::Button { code, pressed } => {
                let Some(button) = pointer_button(code) else {
                    return;
                };
                let position = self.pointer();
                if pressed {
                    self.pressed_at(position);
                }
                let (window, pos) = self.at(position);
                self.push(
                    Incoming::points(beui::Input::PointerButton {
                        pos,
                        button,
                        pressed,
                    })
                    .in_window(window),
                );
            }
            RawInput::Wheel { x, y } => {
                let notches = |v120: i32| -v120 as f32 / 120.0;
                self.scroll(ScrollDelta::Lines(vec2(notches(x), notches(y))));
            }
        }
    }

    fn key(&mut self, code: u32, pressed: bool) {
        let translated = self.keyboard.key(code, pressed);
        if translated.quit {
            self.signal.stop();
            return;
        }
        if let Some(terminal) = translated.terminal {
            if let Err(error) = self.seat.change_vt(terminal) {
                self.problems.report(format!(
                    "Could not switch to terminal {terminal}: {error:?}"
                ));
            }
            return;
        }
        for input in translated.inputs {
            self.push(input);
        }
        match (pressed, translated.repeat) {
            (true, Some(repeated)) => self.start_repeat(code, repeated),
            (false, _) if self.repeat.as_ref().is_some_and(|(held, _)| *held == code) => {
                self.stop_repeat();
            }
            _ => {}
        }
    }

    fn start_repeat(&mut self, code: u32, repeated: Keystroke) {
        self.stop_repeat();
        let interval = self.input.repeat_interval;
        let token = self.handle.insert_source(
            Timer::from_duration(self.input.repeat_delay),
            move |_, _, session| {
                session.push(beui::Input::Key(repeated.clone()));
                TimeoutAction::ToDuration(interval)
            },
        );
        if let Ok(token) = token {
            self.repeat = Some((code, token));
        }
    }

    fn stop_repeat(&mut self) {
        if let Some((_, token)) = self.repeat.take() {
            self.handle.remove(token);
        }
    }
}

impl Card for Session {
    type Output = connector::Handle;

    fn suspend(&mut self) {
        self.libinput.suspend();
        self.drm.pause();
        self.stop_repeat();
        self.stop_filter_timer();
        self.escape = Escape::default();
        if let Some(token) = self.escape_timer.take() {
            self.handle.remove(token);
        }
        if let Some(filter) = self.control.filter() {
            filter.borrow_mut().forget_held();
        }
    }

    fn release(&mut self, output: Option<connector::Handle>) {
        self.outputs
            .borrow_mut()
            .outputs
            .retain(|known| output.is_some_and(|released| released != known.connector));
    }

    fn take_back(&mut self) {
        if self.libinput.resume().is_err() {
            self.problems
                .report("The input devices could not be reopened.".to_owned());
        }
        if let Err(error) = self.drm.activate(true) {
            self.problems.report(format!(
                "The display device could not be taken back: {error}"
            ));
        }
        if let Some((panel, _)) = &mut self.epaper {
            panel.take_over();
        }
        if let Some(keyboard) = Keyboard::new(&self.input) {
            if self.keyboard.modifiers() != Modifiers::NONE {
                self.push(beui::Input::Modifiers(Modifiers::NONE));
            }
            self.keyboard = keyboard;
        }
        if self.blanked {
            self.set_blanked(false);
            self.display_control.woke();
        }
        self.pacer.ask();
    }

    fn rescan(&mut self) {
        self.scan();
    }

    fn stall(&mut self, output: connector::Handle) {
        let mut outputs = self.outputs.borrow_mut();
        if let Some(output) = outputs
            .outputs
            .iter_mut()
            .find(|known| known.connector == output)
        {
            output.stall();
        }
    }

    fn report(&mut self, problem: String) {
        self.problems.report(problem);
    }
}

fn gate_input(event: &InputEvent<LibinputInputBackend>) -> Option<(Input, u64)> {
    let pressed = |held, pressed| match pressed {
        true => Input::Press(held),
        false => Input::Release(held),
    };
    Some(match event {
        InputEvent::Keyboard { event } => (
            pressed(
                Held::Key(event.key_code().raw()),
                event.state() == KeyState::Pressed,
            ),
            event.time(),
        ),
        InputEvent::PointerButton { event } => (
            pressed(
                Held::Button(event.button_code()),
                event.state() == ButtonState::Pressed,
            ),
            event.time(),
        ),
        InputEvent::PointerMotion { event } => (Input::Other, event.time()),
        InputEvent::PointerMotionAbsolute { event } => (Input::Other, event.time()),
        InputEvent::PointerAxis { event } => (Input::Other, event.time()),
        InputEvent::TouchDown { event } => (Input::Press(touch_held(event.slot())), event.time()),
        InputEvent::TouchMotion { event } => {
            (Input::Continue(touch_held(event.slot())), event.time())
        }
        InputEvent::TouchUp { event } => (Input::Release(touch_held(event.slot())), event.time()),
        InputEvent::TouchCancel { event } => {
            (Input::Release(touch_held(event.slot())), event.time())
        }
        InputEvent::TabletToolTip { event } => (
            pressed(
                Held::Button(BTN_TOUCH),
                TabletToolTipEvent::tip_state(event) == TabletToolTipState::Down,
            ),
            event.time(),
        ),
        InputEvent::TabletToolButton { event } => (
            pressed(
                Held::Button(TabletToolButtonEvent::button(event)),
                TabletToolButtonEvent::button_state(event) == ButtonState::Pressed,
            ),
            event.time(),
        ),
        InputEvent::TabletToolAxis { event } => (Input::Other, event.time()),
        InputEvent::TabletToolProximity { event } => (Input::Other, event.time()),
        _ => return None,
    })
}

fn normalized<E: AbsolutePositionEvent<LibinputInputBackend>>(event: &E) -> Pos2 {
    beui::pos2(event.x_transformed(1) as f32, event.y_transformed(1) as f32)
}

fn touch_held(slot: smithay::backend::input::TouchSlot) -> Held {
    Held::Touch(slot.into())
}

fn device_id(device: &Libdevice) -> InputDevice {
    InputDevice {
        name: device.name().to_owned(),
        vendor: device.id_vendor(),
        product: device.id_product(),
    }
}

fn pointer_defaults(device: &Libdevice) -> PointerSettings {
    PointerSettings {
        speed: device
            .config_accel_is_available()
            .then(|| PointerSpeed::new(device.config_accel_default_speed() as f32)),
        tap_to_click: (device.config_tap_finger_count() > 0)
            .then(|| device.config_tap_default_enabled()),
        natural_scroll: device
            .config_scroll_has_natural_scroll()
            .then(|| device.config_scroll_default_natural_scroll_enabled()),
    }
}

fn configure_device(device: &mut Libdevice, config: &InputConfig) {
    let defaults = pointer_defaults(device);
    let wanted = config.pointer(&device_id(device));
    if let Some(default) = defaults.speed {
        let speed = wanted.speed.unwrap_or(default);
        let _ = device.config_accel_set_speed(speed.get().into());
    }
    if let Some(default) = defaults.tap_to_click {
        let _ = device.config_tap_set_enabled(wanted.tap_to_click.unwrap_or(default));
    }
    if let Some(default) = defaults.natural_scroll {
        let _ = device
            .config_scroll_set_natural_scroll_enabled(wanted.natural_scroll.unwrap_or(default));
    }
}

const BTN_TOUCH: u32 = 0x14a;

fn stylus_button(code: u32) -> PointerButton {
    match code {
        0x14b => PointerButton::Secondary,
        0x14c => PointerButton::Middle,
        code => pointer_button(code).unwrap_or(PointerButton::Other(0)),
    }
}

fn pointer_button(code: u32) -> Option<PointerButton> {
    match code {
        0x110 => Some(PointerButton::Primary),
        0x111 => Some(PointerButton::Secondary),
        0x112 => Some(PointerButton::Middle),
        0x113 => Some(PointerButton::Back),
        0x114 => Some(PointerButton::Forward),
        code => u16::try_from(code).ok().map(PointerButton::Other),
    }
}
