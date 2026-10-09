use std::cell::RefCell;
use std::error::Error;
use std::os::fd::{AsFd, BorrowedFd, OwnedFd};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use beui::{
    CursorIcon, Event, FilePickRequest, Launch, Platform, PointerButton, Pos2, Setup, TouchId,
    TouchPhase, Waker, vec2,
};
use beui_core::app::SafeArea;
use beui_core::renderer::Loaded;
use beui_core::runner::Runner;
use smithay::backend::allocator::gbm::GbmDevice;
use smithay::backend::drm::{DrmDevice, DrmDeviceFd, DrmEvent};
use smithay::backend::input::Event as _;
use smithay::backend::input::{
    AbsolutePositionEvent, Axis, ButtonState, InputEvent, KeyState, KeyboardKeyEvent,
    PointerAxisEvent, PointerButtonEvent, PointerMotionEvent, TouchEvent,
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
use smithay::reexports::input::{Device as InputDevice, Libinput};
use smithay::reexports::rustix::fs::OFlags;
use smithay::utils::DeviceFd;

use be_dmabuf::{adapter_for, open_device};

use crate::display::{DisplayConfig, DisplayControl};
use crate::displays::{DisplayRenderer, Displays};
use crate::gpu::{Gpu, SoftwareCursor};
use crate::input::{DeviceId, InputConfig, InputControl, PointerConfig, PointerDevice};
use crate::keyboard::Keyboard;
use crate::layout::{arrange, bounds, clamp, moved};
use crate::output::{Output, connected, wait_for};
use crate::problems::Problems;
use crate::screen::FORMAT;
use crate::wake::{Held, Input, WakeGate};

const WHEEL_STEP: f64 = 15.0;
const LONGEST_WAIT: Duration = Duration::from_secs(3600);
const BLANKED_FRAME: Duration = Duration::from_secs(1);
const DARKEN_RETRY: Duration = Duration::from_secs(1);

struct Session {
    seat: LibSeatSession,
    active: bool,
    handle: LoopHandle<'static, Session>,
    signal: LoopSignal,
    drm: DrmDevice,
    gbm: GbmDevice<DrmDeviceFd>,
    node: u64,
    libinput: Libinput,
    devices: Vec<InputDevice>,
    input: InputConfig,
    control: InputControl,
    display_control: DisplayControl,
    display_config: DisplayConfig,
    modes_pending: bool,
    blanked: bool,
    gate: WakeGate,
    runner: Runner,
    platform: Seat,
    displays: Rc<RefCell<Displays>>,
    dirty: bool,
    keyboard: Keyboard,
    repeat: Option<(u32, RegistrationToken)>,
    wakeup: Option<RegistrationToken>,
    darken_retry: Option<RegistrationToken>,
    lost: Arc<AtomicBool>,
    problems: Problems,
}

struct Seat {
    clipboard: Option<String>,
    locked: bool,
    displays: Rc<RefCell<Displays>>,
}

impl Platform for Seat {
    fn copy(&mut self, text: String) {
        self.clipboard = Some(text);
    }

    fn paste(&mut self) -> Option<String> {
        self.clipboard.clone()
    }

    fn pick_file(&mut self, _request: FilePickRequest) {}

    fn set_cursor(&mut self, icon: CursorIcon, _touch_emulation: bool) {
        self.displays.borrow_mut().icon = icon;
    }

    fn lock_pointer(&mut self, locked: bool) {
        self.locked = locked;
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
    let gbm = GbmDevice::new(fd)?;
    handle.insert_source(drm_events, |event, _, session| match event {
        DrmEvent::VBlank(crtc) => session.flipped(crtc),
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
    handle.insert_source(pinged, |_, _, session| session.dirty = true)?;
    let waker = Waker::new(move || ping.ping());
    let control = InputControl::new(waker.clone());
    let display_control = DisplayControl::new(waker.clone());

    let lost = Arc::new(AtomicBool::new(false));
    let (device, queue) = open_gpu(node, &lost)?;
    let gpu = Rc::new(Gpu::new(device, queue, FORMAT));
    let displays = Rc::new(RefCell::new(Displays::new(
        gpu,
        SoftwareCursor::default(),
        scale(),
    )));
    let mut runner = Runner::new(launch);
    let mut setup = Setup::new(waker);
    setup.provide(control.clone());
    setup.provide(display_control.clone());
    let problems = Problems::default();
    setup.provide(problems.clone());
    runner.start(
        vec![Loaded {
            renderer: Box::new(DisplayRenderer(Rc::clone(&displays))),
            fonts: None,
        }],
        setup,
    )?;
    let input = InputConfig::default();
    let platform = Seat {
        clipboard: None,
        locked: false,
        displays: Rc::clone(&displays),
    };
    let mut session = Session {
        seat,
        active: true,
        handle: handle.clone(),
        signal: event_loop.get_signal(),
        drm,
        gbm,
        node,
        libinput,
        devices: Vec::new(),
        keyboard: Keyboard::new(&input).ok_or("the keymap could not be compiled")?,
        input,
        control,
        display_control,
        display_config: DisplayConfig::default(),
        modes_pending: false,
        blanked: false,
        gate: WakeGate::default(),
        runner,
        platform,
        displays,
        dirty: true,
        repeat: None,
        wakeup: None,
        darken_retry: None,
        lost,
        problems,
    };
    if let Some(config) = session.display_control.take() {
        session.display_config = config;
    }
    session.scan();
    if session.displays.borrow().outputs.is_empty() {
        return Err("no display is connected".into());
    }
    {
        let mut displays = session.displays.borrow_mut();
        displays.pointer = displays.outputs[0].screen.rect.center();
    }
    let ran = event_loop.run(None, &mut session, Session::idle);
    session.end();
    drop(event_loop);
    ran?;
    Ok(())
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
        required_features: wgpu::Features::empty(),
        required_limits: adapter.limits(),
        experimental_features: wgpu::ExperimentalFeatures::disabled(),
        memory_hints: wgpu::MemoryHints::Performance,
        trace: wgpu::Trace::Off,
    };
    let (device, queue) =
        open_device(&adapter, &descriptor).ok_or("the Vulkan device could not be opened")?;
    let flag = lost.clone();
    device.set_device_lost_callback(move |_, message| {
        eprintln!("beui: the GPU was lost: {message}");
        flag.store(true, Ordering::SeqCst);
    });
    Ok((device, queue))
}

impl Session {
    fn end(mut self) {
        self.runner.exit();
        let Session {
            runner,
            platform,
            displays,
            devices,
            libinput,
            gbm,
            drm,
            ..
        } = self;
        drop(runner);
        drop(platform);
        drop(displays);
        drop(devices);
        libinput.suspend();
        drop(libinput);
        drop(gbm);
        drop(drm);
    }

    fn idle(&mut self) {
        if self.lost.swap(false, Ordering::SeqCst) {
            self.recover();
        }
        if let Some(config) = self.control.take() {
            self.configure(config);
        }
        if let Some(config) = self.display_control.take() {
            self.display_config = config;
            self.modes_pending = true;
        }
        if !self.active {
            return;
        }
        if self.modes_pending {
            self.apply_modes();
        }
        if self.dirty || self.runner.has_events() {
            self.update();
        }
        if let Some(blanked) = self.display_control.take_blanked() {
            self.set_blanked(blanked);
        }
        self.runner.present();
        if self.blanked {
            self.darken();
        }
        self.watch_fences();
    }

    fn apply_modes(&mut self) {
        let mut changed = false;
        let mut waiting = false;
        {
            let mut displays = self.displays.borrow_mut();
            let displays = &mut *displays;
            for output in &mut displays.outputs {
                if output.drawing() {
                    waiting = true;
                    continue;
                }
                let wanted = self.display_config.mode(output.id());
                changed |= output.set_mode(&displays.gpu, wanted);
            }
        }
        self.modes_pending = waiting;
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
        for output in &mut self.displays.borrow_mut().outputs {
            output.set_blanked(blanked);
        }
        self.dirty = true;
    }

    fn darken(&mut self) {
        let lit = {
            let mut displays = self.displays.borrow_mut();
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
                    session.dirty = true;
                    TimeoutAction::Drop
                })
                .ok();
        }
    }

    fn watch_fences(&mut self) {
        let fences: Vec<_> = self
            .displays
            .borrow_mut()
            .outputs
            .iter_mut()
            .filter_map(|output| {
                let (frame, fence) = output.take_fence()?;
                Some((output.crtc, frame, fence))
            })
            .collect();
        for (crtc, frame, fence) in fences {
            let spare = fence.try_clone();
            let watched = self.handle.insert_source(
                Generic::new(fence, Interest::READ, Mode::OneShot),
                move |_, _, session| {
                    session.drawn(crtc, frame);
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
                self.drawn(crtc, frame);
            }
        }
    }

    fn drawn(&mut self, crtc: smithay::reexports::drm::control::crtc::Handle, frame: u64) {
        let mut displays = self.displays.borrow_mut();
        if let Some(output) = displays
            .outputs
            .iter_mut()
            .find(|output| output.crtc == crtc)
        {
            output.drawn(frame);
        }
    }

    fn report_monitors(&self) {
        let monitors = self
            .displays
            .borrow()
            .outputs
            .iter()
            .map(Output::monitor)
            .collect();
        self.display_control.set_monitors(monitors);
    }

    fn rects(&self) -> Vec<beui::Rect> {
        self.displays.borrow().rects()
    }

    fn update(&mut self) {
        self.dirty = false;
        let scale = self.displays.borrow().scale;
        let Some(frame) = self
            .runner
            .frame(&mut self.platform, scale, SafeArea::default())
        else {
            return;
        };
        if frame.close_requested && self.runner.close_requested() {
            self.signal.stop();
        }
        match self.blanked {
            false => {
                self.dirty = frame.deferred || frame.repaint;
                self.schedule(frame.repaint_after);
            }
            true => {
                self.dirty = frame.deferred;
                self.schedule(frame.repaint_after.max(BLANKED_FRAME));
            }
        }
    }

    fn schedule(&mut self, after: Duration) {
        if let Some(token) = self.wakeup.take() {
            self.handle.remove(token);
        }
        if after == Duration::MAX {
            return;
        }
        self.wakeup = self
            .handle
            .insert_source(
                Timer::from_duration(after.min(LONGEST_WAIT)),
                |_, _, session| {
                    session.wakeup = None;
                    session.dirty = true;
                    TimeoutAction::Drop
                },
            )
            .ok();
    }

    fn flipped(&mut self, crtc: smithay::reexports::drm::control::crtc::Handle) {
        let mut displays = self.displays.borrow_mut();
        if let Some(output) = displays
            .outputs
            .iter_mut()
            .find(|output| output.crtc == crtc)
        {
            output.flipped();
        }
    }

    fn scan(&mut self) {
        let connectors = connected(&self.drm);
        let mut guard = self.displays.borrow_mut();
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
            match Output::new(
                &mut self.drm,
                &self.gbm,
                &gpu,
                handle,
                &info,
                &taken,
                &self.display_config,
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
        let mut guard = self.displays.borrow_mut();
        let displays = &mut *guard;
        let sizes: Vec<_> = displays
            .outputs
            .iter()
            .map(|output| output.screen.size())
            .collect();
        for (output, rect) in displays
            .outputs
            .iter_mut()
            .zip(arrange(&sizes, displays.scale))
        {
            output.screen.rect = rect;
            output.screen.invalidate();
        }
        displays.pointer = clamp(displays.pointer, &displays.rects());
        self.dirty = true;
        drop(guard);
        self.report_monitors();
    }

    fn recover(&mut self) {
        eprintln!("beui: the GPU was lost, so the session ends");
        self.signal.stop();
    }

    fn seat_event(&mut self, event: SessionEvent) {
        match event {
            SessionEvent::PauseSession => {
                self.active = false;
                self.libinput.suspend();
                self.drm.pause();
                self.stop_repeat();
            }
            SessionEvent::ActivateSession => {
                if self.libinput.resume().is_err() {
                    self.problems
                        .report("The input devices could not be reopened.".to_owned());
                }
                if let Err(error) = self.drm.activate(false) {
                    self.problems.report(format!(
                        "The display device could not be taken back: {error}"
                    ));
                }
                for output in &mut self.displays.borrow_mut().outputs {
                    output.reset();
                }
                self.active = true;
                self.scan();
            }
        }
    }

    fn udev(&mut self, event: UdevEvent) {
        if let UdevEvent::Changed { device_id } = event
            && device_id == self.node
        {
            self.scan();
        }
    }

    fn push(&mut self, event: Event) {
        self.runner.push(event);
        self.dirty = true;
    }

    fn point(&mut self, pointer: Pos2) {
        self.place_pointer(pointer);
        self.push(Event::PointerMoved(pointer));
    }

    fn place_pointer(&mut self, pointer: Pos2) {
        let mut displays = self.displays.borrow_mut();
        displays.pointer = pointer;
        displays.moved_pointer();
    }

    fn gated(&mut self, event: &InputEvent<LibinputInputBackend>) -> bool {
        let Some((input, time)) = gate_input(event) else {
            return true;
        };
        let pass = self.gate.pass(input, time);
        if pass.woke {
            self.set_blanked(false);
            self.display_control.woke();
            self.dirty = true;
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
                _ => {}
            }
        }
        pass.deliver
    }

    fn absolute_position(
        &self,
        event: &<LibinputInputBackend as smithay::backend::input::InputBackend>::PointerMotionAbsoluteEvent,
    ) -> Pos2 {
        let area = bounds(&self.rects());
        let position =
            event.position_transformed((area.width() as i32, area.height() as i32).into());
        clamp(
            area.min + vec2(position.x as f32, position.y as f32),
            &self.rects(),
        )
    }

    fn pointer(&self) -> Pos2 {
        self.displays.borrow().pointer
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
            if defaults == PointerConfig::default() {
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
    }

    fn input(&mut self, event: InputEvent<LibinputInputBackend>) {
        if !self.gated(&event) {
            return;
        }
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
                self.key(code, event.state() == KeyState::Pressed);
            }
            InputEvent::PointerMotion { event } => {
                let delta = vec2(event.delta_x() as f32, event.delta_y() as f32);
                if self.platform.locked {
                    self.push(Event::PointerMotion(delta));
                    return;
                }
                self.point(moved(self.pointer(), delta, &self.rects()));
            }
            InputEvent::PointerMotionAbsolute { event } => {
                let position = self.absolute_position(&event);
                self.point(position);
            }
            InputEvent::PointerButton { event } => {
                let Some(button) = pointer_button(event.button_code()) else {
                    return;
                };
                self.push(Event::PointerButton {
                    pos: self.pointer(),
                    button,
                    pressed: event.state() == ButtonState::Pressed,
                    modifiers: self.keyboard.modifiers(),
                });
            }
            InputEvent::PointerAxis { event } => {
                let amount = |axis| {
                    event
                        .amount(axis)
                        .or_else(|| {
                            event
                                .amount_v120(axis)
                                .map(|v120| v120 / 120.0 * WHEEL_STEP)
                        })
                        .unwrap_or(0.0) as f32
                };
                let delta = vec2(-amount(Axis::Horizontal), -amount(Axis::Vertical));
                if delta != beui::Vec2::ZERO {
                    self.push(Event::Scroll(delta));
                }
            }
            InputEvent::TouchDown { event } => {
                let position = self.touch_position(&event);
                self.touch(event.slot(), TouchPhase::Start, position);
            }
            InputEvent::TouchMotion { event } => {
                let position = self.touch_position(&event);
                self.touch(event.slot(), TouchPhase::Move, position);
            }
            InputEvent::TouchUp { event } => {
                self.touch(event.slot(), TouchPhase::End, self.pointer());
            }
            InputEvent::TouchCancel { event } => {
                self.touch(event.slot(), TouchPhase::Cancel, self.pointer());
            }
            _ => {}
        }
    }

    fn touch_position<E: AbsolutePositionEvent<LibinputInputBackend>>(&self, event: &E) -> Pos2 {
        let first = self.rects().first().copied().unwrap_or(beui::Rect::ZERO);
        let position =
            event.position_transformed((first.width() as i32, first.height() as i32).into());
        first.min + vec2(position.x as f32, position.y as f32)
    }

    fn touch(
        &mut self,
        slot: smithay::backend::input::TouchSlot,
        phase: TouchPhase,
        position: Pos2,
    ) {
        let finger: i32 = slot.into();
        self.push(Event::Touch {
            id: TouchId {
                device: 0,
                finger: u64::from(finger.max(0).unsigned_abs()),
            },
            phase,
            pos: position,
            force: None,
        });
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
        for event in translated.events {
            self.push(event);
        }
        match (pressed, translated.repeat) {
            (true, Some(repeated)) => self.start_repeat(code, repeated),
            (false, _) if self.repeat.as_ref().is_some_and(|(held, _)| *held == code) => {
                self.stop_repeat();
            }
            _ => {}
        }
    }

    fn start_repeat(&mut self, code: u32, repeated: Vec<Event>) {
        self.stop_repeat();
        let interval = self.input.repeat_interval;
        let token = self.handle.insert_source(
            Timer::from_duration(self.input.repeat_delay),
            move |_, _, session| {
                for event in &repeated {
                    session.push(event.clone());
                }
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
        _ => return None,
    })
}

fn touch_held(slot: smithay::backend::input::TouchSlot) -> Held {
    Held::Touch(slot.into())
}

fn device_id(device: &InputDevice) -> DeviceId {
    DeviceId {
        name: device.name().to_owned(),
        vendor: device.id_vendor(),
        product: device.id_product(),
    }
}

fn pointer_defaults(device: &InputDevice) -> PointerConfig {
    PointerConfig {
        speed: device
            .config_accel_is_available()
            .then(|| device.config_accel_default_speed()),
        tap_to_click: (device.config_tap_finger_count() > 0)
            .then(|| device.config_tap_default_enabled()),
        natural_scroll: device
            .config_scroll_has_natural_scroll()
            .then(|| device.config_scroll_default_natural_scroll_enabled()),
    }
}

fn configure_device(device: &mut InputDevice, config: &InputConfig) {
    let defaults = pointer_defaults(device);
    let wanted = config.pointer(&device_id(device));
    if let Some(default) = defaults.speed {
        let _ = device.config_accel_set_speed(wanted.speed.unwrap_or(default));
    }
    if let Some(default) = defaults.tap_to_click {
        let _ = device.config_tap_set_enabled(wanted.tap_to_click.unwrap_or(default));
    }
    if let Some(default) = defaults.natural_scroll {
        let _ = device
            .config_scroll_set_natural_scroll_enabled(wanted.natural_scroll.unwrap_or(default));
    }
}

fn pointer_button(code: u32) -> Option<PointerButton> {
    match code {
        0x110 => Some(PointerButton::Primary),
        0x111 => Some(PointerButton::Secondary),
        0x112 => Some(PointerButton::Middle),
        0x113 | 0x116 => Some(PointerButton::Back),
        0x114 | 0x115 => Some(PointerButton::Forward),
        _ => None,
    }
}
