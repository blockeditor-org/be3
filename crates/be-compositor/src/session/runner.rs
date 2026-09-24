use std::error::Error;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use beui::{
    App as _, Context, Event, FrameOutput, PointerButton, Pos2, RawInput, Repaint, TouchId,
    TouchPhase, Waker, vec2,
};
use smithay::backend::allocator::gbm::GbmDevice;
use smithay::backend::drm::{DrmDevice, DrmDeviceFd, DrmEvent};
use smithay::backend::input::{
    AbsolutePositionEvent, Axis, ButtonState, InputEvent, KeyState, KeyboardKeyEvent,
    PointerAxisEvent, PointerButtonEvent, PointerMotionEvent, TouchEvent,
};
use smithay::backend::libinput::{LibinputInputBackend, LibinputSessionInterface};
use smithay::backend::session::libseat::LibSeatSession;
use smithay::backend::session::{Event as SessionEvent, Session as _};
use smithay::backend::udev::{UdevBackend, UdevEvent, all_gpus, primary_gpu};
use smithay::reexports::calloop::ping::make_ping;
use smithay::reexports::calloop::timer::{TimeoutAction, Timer};
use smithay::reexports::calloop::{EventLoop, LoopHandle, LoopSignal, RegistrationToken};
use smithay::reexports::input::Libinput;
use smithay::reexports::rustix::fs::OFlags;
use smithay::utils::DeviceFd;

use super::arrow;
use super::keyboard::Keyboard;
use super::layout::{arrange, bounds, clamp, moved};
use super::output::{Output, connected};
use super::screen::{FORMAT, Frame};
use crate::app::Compositor;
use crate::gpu::{adapter_for, open_device};
use crate::render::SurfaceTexture;
use crate::server::Server;

const REPEAT_DELAY: Duration = Duration::from_millis(600);
const REPEAT_RATE: Duration = Duration::from_millis(25);
const WHEEL_STEP: f64 = 15.0;

struct Session {
    seat: LibSeatSession,
    active: bool,
    handle: LoopHandle<'static, Session>,
    signal: LoopSignal,
    drm: DrmDevice,
    gbm: GbmDevice<DrmDeviceFd>,
    node: u64,
    libinput: Libinput,
    outputs: Vec<Output>,
    compositor: Compositor,
    context: Context,
    frame: Option<FrameOutput>,
    events: Vec<Event>,
    dirty: bool,
    scale: f32,
    pointer: Pos2,
    keyboard: Keyboard,
    repeat: Option<(u32, RegistrationToken)>,
    wakeup: Option<RegistrationToken>,
    arrow: Rc<SurfaceTexture>,
    lost: Arc<AtomicBool>,
}

pub fn run(launches: Vec<String>) -> Result<(), Box<dyn Error>> {
    let mut event_loop: EventLoop<'static, Session> = EventLoop::try_new()?;
    let handle = event_loop.handle();
    let (mut seat, notifier) =
        LibSeatSession::new().map_err(|error| format!("no seat could be opened: {error:?}"))?;
    handle.insert_source(notifier, |event, _, session| session.seat_event(event))?;
    let name = seat.seat();
    let path = primary_gpu(&name)?
        .or_else(|| all_gpus(&name).ok()?.into_iter().next())
        .ok_or("no GPU was found on this seat")?;
    let fd = seat
        .open(
            &path,
            OFlags::RDWR | OFlags::CLOEXEC | OFlags::NOCTTY | OFlags::NONBLOCK,
        )
        .map_err(|error| format!("{} could not be opened: {error:?}", path.display()))?;
    let fd = DrmDeviceFd::new(DeviceFd::from(fd));
    let (drm, drm_events) = DrmDevice::new(fd.clone(), true)?;
    let node = drm.device_id();
    let gbm = GbmDevice::new(fd)?;
    handle.insert_source(drm_events, |event, _, session| match event {
        DrmEvent::VBlank(crtc) => session.flipped(crtc),
        DrmEvent::Error(error) => eprintln!("be-compositor: the display device failed: {error}"),
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

    let lost = Arc::new(AtomicBool::new(false));
    let (device, queue) = open_gpu(node, &lost)?;
    let mut compositor = Compositor::new(Server::new()?, launches);
    compositor.start(device, queue, FORMAT, Waker::new(move || ping.ping()));
    let gpu = compositor.gpu().ok_or("the compositor has no GPU")?;
    let arrow = Rc::new(gpu.rgba(arrow::WIDTH, arrow::HEIGHT, &arrow::pixels()));
    let context = Context::new();
    let mut session = Session {
        seat,
        active: true,
        handle: handle.clone(),
        signal: event_loop.get_signal(),
        drm,
        gbm,
        node,
        libinput,
        outputs: Vec::new(),
        compositor,
        context,
        frame: None,
        events: Vec::new(),
        dirty: true,
        scale: scale(),
        pointer: Pos2::ZERO,
        keyboard: Keyboard::new().ok_or("the keymap could not be compiled")?,
        repeat: None,
        wakeup: None,
        arrow,
        lost,
    };
    session.scan();
    if session.outputs.is_empty() {
        return Err("no display is connected".into());
    }
    let first = session.outputs[0].screen.rect;
    session.pointer = first.center();
    event_loop.run(None, &mut session, Session::idle)?;
    Ok(())
}

fn scale() -> f32 {
    std::env::var("BE_COMPOSITOR_SCALE")
        .ok()
        .and_then(|scale| scale.parse::<f32>().ok())
        .filter(|scale| *scale > 0.0)
        .unwrap_or(1.0)
}

fn open_gpu(node: u64, lost: &Arc<AtomicBool>) -> Result<(wgpu::Device, wgpu::Queue), String> {
    let adapter = adapter_for(node).ok_or("no Vulkan device drives this display")?;
    let descriptor = wgpu::DeviceDescriptor {
        label: Some("be-compositor"),
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
        eprintln!("be-compositor: the GPU was lost: {message}");
        flag.store(true, Ordering::SeqCst);
    });
    Ok((device, queue))
}

impl Session {
    fn idle(&mut self) {
        if self.lost.swap(false, Ordering::SeqCst) {
            self.recover();
        }
        if !self.active {
            return;
        }
        if self.dirty || !self.events.is_empty() {
            self.update();
        }
        self.render();
    }

    fn rects(&self) -> Vec<beui::Rect> {
        self.outputs
            .iter()
            .map(|output| output.screen.rect)
            .collect()
    }

    fn update(&mut self) {
        self.dirty = false;
        let rect = bounds(&self.rects());
        self.context.set_pixels_per_point(self.scale);
        let raw = RawInput {
            events: std::mem::take(&mut self.events),
        };
        let compositor = &mut self.compositor;
        let output = self
            .context
            .run(raw, |context| compositor.update(context, rect));
        if output.close_requested {
            self.signal.stop();
        }
        if output.changed {
            let clear = self.compositor.clear_color();
            let repaint = match output.damage() {
                Some(region) => Repaint::Region {
                    region,
                    background: clear,
                },
                None => Repaint::Everything,
            };
            for output in &mut self.outputs {
                output.screen.damage(repaint);
            }
        }
        self.schedule(output.repaint_after);
        self.frame = Some(output);
    }

    fn schedule(&mut self, after: Duration) {
        if let Some(token) = self.wakeup.take() {
            self.handle.remove(token);
        }
        if after >= Duration::from_secs(3600) {
            return;
        }
        self.wakeup = self
            .handle
            .insert_source(Timer::from_duration(after), |_, _, session| {
                session.wakeup = None;
                session.dirty = true;
                TimeoutAction::Drop
            })
            .ok();
    }

    fn render(&mut self) {
        let Some(frame) = self.frame.as_ref() else {
            return;
        };
        let Some(gpu) = self.compositor.gpu() else {
            return;
        };
        let sprite = self.compositor.sprite(frame.cursor_icon);
        let clear = self.compositor.clear_color();
        for output in &mut self.outputs {
            if !output.wants_frame() {
                continue;
            }
            let drawn = output.render(
                &gpu,
                Frame {
                    output: frame,
                    scale: self.scale,
                    clear,
                    pointer: self.pointer,
                    sprite: &sprite,
                    arrow: &self.arrow,
                },
            );
            if let Err(error) = drawn {
                eprintln!("be-compositor: an output could not be drawn: {error}");
            }
        }
    }

    fn flipped(&mut self, crtc: smithay::reexports::drm::control::crtc::Handle) {
        if let Some(output) = self.outputs.iter_mut().find(|output| output.crtc == crtc) {
            output.flipped();
        }
    }

    fn scan(&mut self) {
        let connectors = connected(&self.drm);
        self.outputs.retain(|output| {
            connectors
                .iter()
                .any(|(handle, _)| *handle == output.connector)
        });
        let Some(gpu) = self.compositor.gpu() else {
            return;
        };
        for (handle, info) in connectors {
            if self.outputs.iter().any(|output| output.connector == handle) {
                continue;
            }
            let taken: Vec<_> = self.outputs.iter().map(|output| output.crtc).collect();
            match Output::new(&mut self.drm, &self.gbm, &gpu, handle, &info, &taken) {
                Ok(output) => self.outputs.push(output),
                Err(error) => eprintln!("be-compositor: a display was skipped: {error}"),
            }
        }
        let sizes: Vec<_> = self
            .outputs
            .iter()
            .map(|output| output.screen.size())
            .collect();
        for (output, rect) in self.outputs.iter_mut().zip(arrange(&sizes, self.scale)) {
            output.screen.rect = rect;
            output.screen.invalidate();
        }
        self.pointer = clamp(self.pointer, &self.rects());
        self.dirty = true;
    }

    fn recover(&mut self) {
        match open_gpu(self.node, &self.lost) {
            Ok((device, queue)) => {
                self.compositor.replace_gpu(device, queue, FORMAT);
                let Some(gpu) = self.compositor.gpu() else {
                    return;
                };
                self.arrow = Rc::new(gpu.rgba(arrow::WIDTH, arrow::HEIGHT, &arrow::pixels()));
                for output in &mut self.outputs {
                    output.replace_gpu(&gpu);
                }
                self.dirty = true;
            }
            Err(error) => {
                eprintln!("be-compositor: the GPU could not be reopened: {error}");
                self.signal.stop();
            }
        }
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
                    eprintln!("be-compositor: the input devices could not be reopened");
                }
                if let Err(error) = self.drm.activate(false) {
                    eprintln!("be-compositor: the display device could not be reclaimed: {error}");
                }
                for output in &mut self.outputs {
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
        self.events.push(event);
        self.dirty = true;
    }

    fn moved_pointer(&mut self) {
        for output in &mut self.outputs {
            output.screen.cursor = true;
        }
    }

    fn input(&mut self, event: InputEvent<LibinputInputBackend>) {
        match event {
            InputEvent::Keyboard { event } => {
                let code = event.key_code().raw().saturating_sub(8);
                self.key(code, event.state() == KeyState::Pressed);
            }
            InputEvent::PointerMotion { event } => {
                let delta = vec2(event.delta_x() as f32, event.delta_y() as f32);
                if self
                    .frame
                    .as_ref()
                    .is_some_and(|frame| frame.pointer_locked)
                {
                    self.push(Event::PointerMotion(delta));
                    return;
                }
                self.pointer = moved(self.pointer, delta, &self.rects());
                self.push(Event::PointerMoved(self.pointer));
                self.moved_pointer();
            }
            InputEvent::PointerMotionAbsolute { event } => {
                let area = bounds(&self.rects());
                let position =
                    event.position_transformed((area.width() as i32, area.height() as i32).into());
                self.pointer = clamp(
                    area.min + vec2(position.x as f32, position.y as f32),
                    &self.rects(),
                );
                self.push(Event::PointerMoved(self.pointer));
                self.moved_pointer();
            }
            InputEvent::PointerButton { event } => {
                let Some(button) = pointer_button(event.button_code()) else {
                    return;
                };
                self.push(Event::PointerButton {
                    pos: self.pointer,
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
                self.touch(event.slot(), TouchPhase::End, self.pointer);
            }
            InputEvent::TouchCancel { event } => {
                self.touch(event.slot(), TouchPhase::Cancel, self.pointer);
            }
            _ => {}
        }
    }

    fn touch_position<E: AbsolutePositionEvent<LibinputInputBackend>>(&self, event: &E) -> Pos2 {
        let first = self
            .outputs
            .first()
            .map_or(beui::Rect::ZERO, |output| output.screen.rect);
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
                eprintln!("be-compositor: could not switch to terminal {terminal}: {error:?}");
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
        let token =
            self.handle
                .insert_source(Timer::from_duration(REPEAT_DELAY), move |_, _, session| {
                    for event in &repeated {
                        session.push(event.clone());
                    }
                    TimeoutAction::ToDuration(REPEAT_RATE)
                });
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
