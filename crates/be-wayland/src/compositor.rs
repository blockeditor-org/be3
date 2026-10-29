use std::cell::RefCell;
use std::process::{Command as Process, Stdio};
use std::rc::Rc;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::{Duration, Instant};

use beui::native::Document;
use beui::reactive::*;
use beui::{
    CursorIcon, Event, Forwarded, ForwardedInput, ImeEvent, NodeId, PointerButton, PointerId,
    PointerPhase, Pos2, Rect, Vec2, native::Context, native::Waker,
};
use smithay::backend::renderer::utils::with_renderer_surface_state;
use smithay::input::pointer::{CursorImageStatus, CursorImageSurfaceData};
use smithay::wayland::compositor::with_states;

use crate::presentation::{Place, Presented};
use crate::render::{Gpu, Textures, WindowDraw};
use crate::server::{Server, Watch};
use crate::state::{KeyboardConfig, ServerEvent, WindowId};
use crate::windows::{Command, Launch, Popups, WindowInfo, Windows};

const BUTTON_LEFT: u32 = 0x110;
const BUTTON_RIGHT: u32 = 0x111;
const BUTTON_MIDDLE: u32 = 0x112;
const BUTTON_SIDE: u32 = 0x113;
const BUTTON_EXTRA: u32 = 0x114;
const MAX_ANCESTORS: usize = 32;

pub struct CursorImage {
    pub texture: wgpu::Texture,
    pub size: Vec2,
    pub hotspot: Vec2,
    pub opaque: bool,
    pub revision: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Monitor {
    pub id: String,
    pub name: String,
    pub rect: Rect,
    pub scale: f32,
}

pub struct Compositor {
    server: Server,
    windows: Windows,
    textures: Rc<RefCell<Textures>>,
    watch: Option<Watch>,
    waker: Option<Waker>,
    children: Vec<(rustix::process::Pid, Option<u64>)>,
    origins: RefCell<std::collections::HashMap<WindowId, Origin>>,
    exited: (Sender<Exit>, Receiver<Exit>),
    on_failure: Option<Box<dyn Fn(String)>>,
    configured: std::collections::HashMap<WindowId, Configured>,
    area: Rect,
    scale: f32,
    monitors: Vec<Monitor>,
    fullscreen: Option<(WindowId, Pos2)>,
    raise: Option<WindowId>,
    relist: bool,
    input_frames: u64,
    forced: bool,
    listed: u64,
    locked: bool,
    clipboard: clipboard::Clipboard,
    gtk_profile: Option<std::path::PathBuf>,
    cursor_surface: Option<smithay::reexports::wayland_server::protocol::wl_surface::WlSurface>,
    cursor_revision: u64,
    unowned: unowned::Unowned,
}

#[derive(Clone, Copy, PartialEq)]
struct Configured {
    size: (i32, i32),
    activated: bool,
    fullscreen: bool,
}

impl Compositor {
    pub fn new(server: Server, windows: Windows) -> Self {
        let gtk_profile = server
            .socket_name()
            .and(server.runtime_dir())
            .and_then(crate::gtk_settings::profile);
        Self {
            server,
            windows,
            textures: Rc::new(RefCell::new(Textures::default())),
            watch: None,
            waker: None,
            children: Vec::new(),
            origins: RefCell::new(std::collections::HashMap::new()),
            exited: channel(),
            on_failure: None,
            configured: std::collections::HashMap::new(),
            area: Rect::ZERO,
            scale: 1.0,
            monitors: Vec::new(),
            fullscreen: None,
            raise: None,
            relist: false,
            input_frames: 0,
            forced: false,
            listed: 0,
            locked: false,
            clipboard: clipboard::Clipboard::default(),
            gtk_profile,
            cursor_surface: None,
            cursor_revision: 0,
            unowned: unowned::Unowned::default(),
        }
    }

    pub fn server(&mut self) -> &mut Server {
        &mut self.server
    }

    pub fn set_keyboard(&mut self, keyboard: &KeyboardConfig) -> bool {
        self.server.state.set_keyboard(keyboard)
    }

    pub fn set_blank_after(&mut self, after: Option<Duration>) {
        self.server.state.idle.set_blank_after(after);
    }

    pub fn set_lock_after(&mut self, after: Option<Duration>) {
        self.server.state.idle.set_lock_after(after);
    }

    pub fn lock_due(&self) -> bool {
        self.server.state.idle.lock_due()
    }

    pub fn set_locked(&mut self, locked: bool) {
        if self.locked == locked {
            return;
        }
        self.locked = locked;
        if !locked {
            return;
        }
        self.server.state.dismiss_popups();
        self.server.state.release_buttons();
        if self.server.state.pointer_window().is_some() {
            self.server.state.pointer_motion(None);
        }
        self.server.flush();
    }

    pub fn locked(&self) -> bool {
        self.locked
    }

    pub fn woke(&mut self) {
        self.server.state.idle.woke();
    }

    pub fn idle(&self) -> bool {
        self.server.state.idle.blanked()
    }

    pub fn set_monitors(&mut self, monitors: Vec<Monitor>) {
        self.monitors = monitors;
    }

    fn shown_monitors(&self) -> Vec<Monitor> {
        match self.monitors.is_empty() {
            false => self.monitors.clone(),
            true => vec![Monitor {
                id: "window".to_owned(),
                name: "Window".to_owned(),
                rect: self.area,
                scale: self.scale,
            }],
        }
    }

    pub fn set_fullscreen(&mut self, id: WindowId, fullscreen: bool) {
        let current = self.fullscreen.map(|(id, _)| id);
        if fullscreen {
            if let Some(previous) = current.filter(|previous| *previous != id) {
                self.server.state.set_fullscreen(previous, false);
                self.windows.push(Command::Configure(previous));
            }
            let asked = self.server.state.take_fullscreen_output(id).map(|on| {
                Pos2::new(
                    on.loc.x as f32 + on.size.w as f32 / 2.0,
                    on.loc.y as f32 + on.size.h as f32 / 2.0,
                )
            });
            if current != Some(id) || asked.is_some() {
                let anchor = asked.unwrap_or_else(|| {
                    self.windows
                        .rect(id)
                        .map_or(self.area.center(), |rect| rect.center())
                });
                self.fullscreen = Some((id, anchor));
            }
            self.raise = Some(id);
        } else if current == Some(id) {
            self.fullscreen = None;
        }
        self.server.state.set_fullscreen(id, fullscreen);
        self.windows.push(Command::Configure(id));
        self.relist = true;
    }

    fn fullscreen_area(&self, id: WindowId) -> Option<Rect> {
        let (shown, anchor) = self.fullscreen?;
        if shown != id {
            return None;
        }
        let area = self.area;
        Some(
            self.monitors
                .iter()
                .find(|monitor| monitor.rect.contains(anchor))
                .map_or(area, |monitor| monitor.rect.intersect(area)),
        )
    }

    fn publish(&mut self) {
        for (id, fullscreen) in self.windows.take_fullscreen_requests() {
            self.set_fullscreen(id, fullscreen);
        }
        if let Some((id, _)) = self.fullscreen
            && !self.server.state.windows().contains(&id)
        {
            self.fullscreen = None;
            self.relist = true;
        }
        let list =
            (std::mem::take(&mut self.relist) || self.fullscreen.is_some()).then(|| self.list());
        let raise = self.raise.take().or_else(|| self.windows.take_activation());
        let raise = raise.filter(|_| !self.locked);
        let windows = self.windows;
        if let Some(list) = list {
            windows.set_list(list);
        }
        if let Some(id) = raise {
            windows.raise(id);
        }
    }

    pub fn keyboard_window(&self) -> Option<WindowInfo> {
        let id = self.server.state.keyboard_window()?;
        self.windows
            .list()
            .with_untracked(|windows| windows.iter().find(|window| window.id == id).cloned())
    }

    pub fn windows(&self) -> Windows {
        self.windows
    }

    pub fn socket(&self) -> Option<String> {
        self.server
            .socket_name()
            .map(|name| name.to_string_lossy().into_owned())
    }

    pub fn x11_display(&self) -> Option<String> {
        self.server
            .x11_display()
            .map(|display| format!(":{display}"))
    }

    pub fn environment(&self) -> Vec<(&'static str, String)> {
        let runtime = self
            .server
            .runtime_dir()
            .map(|dir| ("XDG_RUNTIME_DIR", dir.to_string_lossy().into_owned()));
        let wayland = self.socket().map(|socket| ("WAYLAND_DISPLAY", socket));
        let x11 = self.x11_display().map(|display| ("DISPLAY", display));
        let gtk = self
            .gtk_profile
            .as_ref()
            .map(|profile| ("DCONF_PROFILE", profile.to_string_lossy().into_owned()));
        runtime
            .into_iter()
            .chain(wayland)
            .chain(x11)
            .chain(gtk)
            .collect()
    }

    fn launch(&mut self, launch: &Launch, now: Instant) {
        let Some((program, arguments)) = launch.arguments.split_first() else {
            return;
        };
        let token = self.server.state.activation.launch_token(now);
        let mut process = Process::new(program);
        if let Some(dir) = &launch.working_dir {
            process.current_dir(dir);
        }
        for variable in beui_host::native::environment::VARIABLES {
            process.env_remove(variable);
        }
        process
            .args(arguments)
            .env("XDG_ACTIVATION_TOKEN", &token)
            .env("DESKTOP_STARTUP_ID", &token)
            .env_remove("WAYLAND_DISPLAY")
            .env_remove("DISPLAY")
            .envs(self.environment())
            .env("GDK_BACKEND", "wayland")
            .env("QT_QPA_PLATFORM", "wayland")
            .env("SDL_VIDEODRIVER", "wayland")
            .env("MOZ_ENABLE_WAYLAND", "1")
            .env("ELECTRON_OZONE_PLATFORM_HINT", "wayland")
            .stdin(Stdio::null());
        let mut child = match process.spawn() {
            Ok(child) => child,
            Err(error) => {
                self.fail(format!("Could not run {}: {error}", launch_name(launch)));
                return;
            }
        };
        let pid = rustix::process::Pid::from_child(&child);
        self.children.push((pid, launch.tag));
        let exited = self.exited.0.clone();
        let waker = self.waker.clone();
        let line = launch_name(launch);
        std::thread::spawn(move || {
            let status = child.wait().ok().and_then(|status| status.code());
            let _ = exited.send(Exit { pid, line, status });
            if let Some(waker) = waker {
                waker.wake();
            }
        });
    }

    pub fn on_failure(&mut self, report: impl Fn(String) + 'static) {
        self.on_failure = Some(Box::new(report));
    }

    fn fail(&self, problem: String) {
        eprintln!("be-wayland: {problem}");
        if let Some(report) = &self.on_failure {
            report(problem);
        }
    }

    fn reap(&mut self) {
        while let Ok(exit) = self.exited.1.try_recv() {
            self.children.retain(|(pid, _)| *pid != exit.pid);
            if let Some(problem) = exit.problem() {
                self.fail(problem);
            }
        }
    }

    fn receive(&mut self, context: &Context) {
        self.server.state.activation.tick(context.now());
        self.server.dispatch();
        self.sync_clipboard(context);
        self.watch_pings(context);
        let events = self.server.state.take_events();
        let cursor = match self.server.state.cursor() {
            CursorImageStatus::Surface(surface) => Some(surface.clone()),
            _ => None,
        };
        let mut cursor_changed = cursor != self.cursor_surface;
        for surface in self.server.state.take_committed() {
            self.textures.borrow_mut().upload(&surface);
            cursor_changed |= cursor.as_ref() == Some(&surface);
        }
        if cursor_changed {
            self.cursor_surface = cursor;
            self.cursor_revision += 1;
        }
        self.textures.borrow_mut().prune();
        let mut redraw = Vec::new();
        let mut changed = false;
        let mut fullscreen = Vec::new();
        let mut activated = None;
        let windows = self.windows;
        for event in events {
            match event {
                ServerEvent::Opened(id) => windows.open(id),
                ServerEvent::Closed(id) => {
                    windows.forget(id);
                    changed = true;
                }
                ServerEvent::Titled(..) | ServerEvent::Changed(_) => changed = true,
                ServerEvent::Fullscreen(id, on) => fullscreen.push((id, on)),
                ServerEvent::Activated(id) => activated = Some(id),
                ServerEvent::Committed(id) => {
                    if !windows.listed(id) {
                        changed = true;
                    }
                    if !redraw.contains(&id) {
                        redraw.push(id);
                    }
                }
            }
        }
        for (id, on) in fullscreen {
            self.set_fullscreen(id, on);
        }
        if let Some(id) = activated {
            self.windows.activate(id);
        }
        let open = self.server.state.windows();
        self.configured.retain(|id, _| open.contains(id));
        self.origins.borrow_mut().retain(|id, _| open.contains(id));
        if changed {
            let list = self.list();
            let windows = self.windows;
            windows.set_list(list);
        }
        if !redraw.is_empty() {
            self.redraw(redraw);
        }
    }

    fn watch_pings(&mut self, context: &Context) {
        let now = context.now();
        if let Some(due) = self.server.state.check_pings(now) {
            context.request_repaint_after(due.saturating_duration_since(now));
        }
    }

    fn force_close(&mut self, id: WindowId) {
        let pid = self
            .server
            .state
            .client_pid(id)
            .and_then(rustix::process::Pid::from_raw);
        if let Some(pid) = pid {
            self.end_launched(pid);
        }
        self.server.state.disconnect(id);
        self.forced = true;
    }

    fn end_launched(&self, pid: rustix::process::Pid) -> bool {
        let Ok(process) = rustix::process::pidfd_open(pid, rustix::process::PidfdFlags::empty())
        else {
            return false;
        };
        self.launched(pid)
            && rustix::process::pidfd_send_signal(&process, rustix::process::Signal::KILL).is_ok()
    }

    fn launched(&self, pid: rustix::process::Pid) -> bool {
        self.launch_of(pid).is_some()
    }

    fn launch_of(&self, pid: rustix::process::Pid) -> Option<Option<u64>> {
        let mut pid = pid;
        for _ in 0..MAX_ANCESTORS {
            if let Some((_, tag)) = self.children.iter().find(|(child, _)| *child == pid) {
                return Some(*tag);
            }
            pid = parent(pid)?;
        }
        None
    }

    fn origin(&self, id: WindowId) -> Origin {
        if let Some(origin) = self.origins.borrow().get(&id) {
            return origin.clone();
        }
        let pid = self
            .server
            .state
            .client_pid(id)
            .and_then(rustix::process::Pid::from_raw);
        let origin = Origin {
            launch: pid.and_then(|pid| self.launch_of(pid)).flatten(),
            command: pid.map(command_line).unwrap_or_default(),
        };
        self.origins.borrow_mut().insert(id, origin.clone());
        origin
    }

    fn list(&self) -> Vec<WindowInfo> {
        let state = &self.server.state;
        state
            .windows()
            .into_iter()
            .filter_map(|id| {
                let size = state.mapped_size(id)?;
                let origin = self.origin(id);
                Some(WindowInfo {
                    id,
                    size,
                    title: state.title(id).unwrap_or_default(),
                    app_id: state.app_id(id).unwrap_or_default(),
                    parent: state.parent(id),
                    modal: state.modal(id),
                    fullscreen: self.fullscreen_area(id),
                    responding: state.responding(id),
                    icon: state.icon(id),
                    inhibits_shortcuts: state.inhibits_shortcuts(id),
                    launch: origin.launch,
                    command: origin.command,
                })
            })
            .collect()
    }

    fn redraw(&mut self, windows: Vec<WindowId>) {
        let drawings: Vec<_> = windows
            .into_iter()
            .map(|id| (id, self.drawing(id)))
            .collect();
        for (id, _) in &drawings {
            let callbacks = self.server.state.take_feedback(*id);
            self.server.state.presentation.drawn(*id, callbacks);
        }
        let store = self.windows;
        for (id, (drawing, popups)) in drawings {
            store.draw(id, drawing, popups);
        }
    }

    pub fn gpu(&self) -> Option<Rc<Gpu>> {
        self.textures.borrow().gpu()
    }

    pub fn cursor_image(&self) -> Option<CursorImage> {
        self.server.state.pointer_window()?;
        let CursorImageStatus::Surface(surface) = self.server.state.cursor() else {
            return None;
        };
        let surface = surface.clone();
        let current = self.textures.borrow().get(&surface)?;
        let size = with_renderer_surface_state(&surface, |state| state.surface_size()).flatten();
        let hotspot = with_states(&surface, |states| {
            states
                .data_map
                .get::<CursorImageSurfaceData>()
                .map(|data| data.lock().unwrap().hotspot)
        });
        let (size, hotspot) = (size?, hotspot?);
        Some(CursorImage {
            texture: current.texture.texture().clone(),
            size: beui::vec2(size.w as f32, size.h as f32),
            hotspot: beui::vec2(hotspot.x as f32, hotspot.y as f32),
            opaque: current.texture.opaque(),
            revision: self.cursor_revision,
        })
    }

    fn drawing(&self, id: WindowId) -> (Option<beui::native::Drawing>, Option<Popups>) {
        let textures = self.textures.borrow();
        let (Some(gpu), Some(signals)) = (textures.gpu(), self.windows.signals(id)) else {
            return (None, None);
        };
        let (popups, own): (Vec<_>, Vec<_>) = self
            .server
            .state
            .layers(id)
            .into_iter()
            .filter_map(|layer| Some((textures.get(&layer.surface)?, layer)))
            .partition(|(_, layer)| layer.popup);
        let drawing = WindowDraw::drawing(
            gpu.clone(),
            own,
            signals.painted.clone(),
            self.waker.clone(),
        );
        let bounds = popups
            .iter()
            .map(|(_, layer)| layer.rect)
            .reduce(|bounds, rect| bounds.merge(rect));
        let popups = bounds.map(|bounds| {
            let areas = popups
                .iter()
                .map(|(_, layer)| {
                    let at = layer.rect.loc - bounds.loc;
                    Rect::from_min_size(
                        Pos2::new(at.x as f32, at.y as f32),
                        Vec2::new(layer.rect.size.w as f32, layer.rect.size.h as f32),
                    )
                })
                .collect();
            let layers = popups
                .into_iter()
                .map(|(texture, mut layer)| {
                    layer.rect.loc -= bounds.loc;
                    (texture, layer)
                })
                .collect();
            Popups {
                areas,
                rect: Rect::from_min_size(
                    Pos2::new(bounds.loc.x as f32, bounds.loc.y as f32),
                    Vec2::new(bounds.size.w as f32, bounds.size.h as f32),
                ),
                drawing: WindowDraw::drawing(
                    gpu,
                    layers,
                    signals.painted.clone(),
                    self.waker.clone(),
                ),
            }
        });
        (Some(drawing), popups)
    }

    fn input(&mut self, now: Instant) {
        let inputs = self.windows.take_input();
        if !self.owned() {
            return;
        }
        for (id, input) in &inputs {
            let id = *id;
            if !self.server.state.windows().contains(&id) {
                continue;
            }
            let hovered = inputs
                .iter()
                .any(|(other, input)| *other == id && input.hovered);
            match self.windows.blocked_by(id) {
                Some(dialog) => self.refuse(id, dialog, input, now),
                None => self.deliver(id, input, hovered, now),
            }
        }
    }

    fn refuse(&mut self, id: WindowId, dialog: WindowId, input: &ForwardedInput, now: Instant) {
        let pressed = input
            .pointer_events()
            .any(|event| matches!(event.phase, PointerPhase::Down(_)));
        if pressed {
            self.raise = Some(dialog);
        }
        let state = &self.server.state;
        if state.buttons_held() && state.pointer_window() == Some(id) || state.touching(id) {
            let releases = ForwardedInput {
                events: input
                    .events
                    .iter()
                    .filter(|event| {
                        matches!(
                            event,
                            Forwarded::Pointer { event, .. }
                                if matches!(event.phase, PointerPhase::Up(_) | PointerPhase::Cancel)
                        )
                    })
                    .cloned()
                    .collect(),
                ..input.clone()
            };
            self.deliver(id, &releases, false, now);
        }
        let state = &mut self.server.state;
        if !state.buttons_held() && state.pointer_window() == Some(id) {
            state.pointer_motion(None);
        }
    }

    fn deliver(&mut self, id: WindowId, input: &ForwardedInput, hovered: bool, now: Instant) {
        let state = &mut self.server.state;
        let dragging = state.dragging();
        let windows = &self.windows;
        let shift = windows
            .rect(id)
            .map_or(beui::Vec2::ZERO, |desktop| desktop.min - input.rect.min);
        let at = |over: Option<NodeId>, pos: Pos2| {
            let (id, local) = match dragging {
                true => windows
                    .caught_at(over?)
                    .filter(|(target, _)| windows.blocked_by(*target).is_none())
                    .map(|(target, rect)| (target, pos + shift - rect.min))?,
                false => (id, pos - input.rect.min),
            };
            Some((id, (f64::from(local.x), f64::from(local.y)).into()))
        };
        let mut touched = false;
        for forwarded in &input.events {
            match forwarded {
                Forwarded::Input(Event::Key {
                    press,
                    evdev: Some(code),
                    ..
                }) if !press.repeat && state.keyboard_window() == Some(id) => {
                    if press.pressed {
                        state.ping(id, now);
                    }
                    state.key(*code, press.pressed);
                }
                Forwarded::Input(Event::Text(text)) if state.keyboard_window() == Some(id) => {
                    state.text_inputs.commit_text(text);
                }
                Forwarded::Input(Event::Ime(ImeEvent::CommitText(text)))
                    if state.keyboard_window() == Some(id) =>
                {
                    state.text_inputs.commit_text(text);
                }
                Forwarded::Pointer { event, over } if event.pointer == PointerId::Mouse => {
                    let over = over.map(|over| over.node);
                    match event.phase {
                        PointerPhase::Move => state.pointer_motion(at(over, event.pos)),
                        PointerPhase::Down(button) | PointerPhase::Up(button) => {
                            let pressed = matches!(event.phase, PointerPhase::Down(_));
                            let target = at(over, event.pos);
                            state.pointer_motion(target);
                            if pressed {
                                state.ping(id, now);
                                if let Some((target, _)) = target {
                                    windows.pressed(target);
                                }
                            }
                            state.pointer_button(button_code(button), pressed);
                        }
                        PointerPhase::Scroll(delta) => {
                            if state.pointer_window() != Some(id) {
                                state.pointer_motion(at(over, event.pos));
                            }
                            state.pointer_axis((-f64::from(delta.x), -f64::from(delta.y)));
                        }
                        _ => {}
                    }
                }
                Forwarded::Pointer { event, over } if event.pointer == PointerId::Pen => {
                    let over = over.map(|over| over.node);
                    let pen = event.pen.unwrap_or_default();
                    let target = at(over, event.pos);
                    state.pen_motion(
                        target,
                        f64::from(pen.pressure),
                        (f64::from(pen.tilt.x), f64::from(pen.tilt.y)),
                        pen.eraser,
                    );
                    match event.phase {
                        PointerPhase::Down(button) | PointerPhase::Up(button) => {
                            let pressed = matches!(event.phase, PointerPhase::Down(_));
                            if pressed {
                                state.ping(id, now);
                                if let Some((target, _)) = target {
                                    windows.pressed(target);
                                }
                            }
                            match button {
                                PointerButton::Primary => state.pen_tip(pressed),
                                button => state.pen_button(button_code(button), pressed),
                            }
                        }
                        PointerPhase::Cancel | PointerPhase::Gone => state.pen_gone(),
                        _ => {}
                    }
                }
                Forwarded::Pointer { event, over } => {
                    let PointerId::Touch(finger) = event.pointer else {
                        continue;
                    };
                    let target = at(over.map(|over| over.node), event.pos);
                    touched = true;
                    match event.phase {
                        PointerPhase::Down(_) => {
                            state.ping(id, now);
                            if let Some((target, _)) = target {
                                windows.pressed(target);
                            }
                            state.touch_seen();
                            state.touch_down(finger, target);
                        }
                        PointerPhase::Move => state.touch_motion(finger, target),
                        PointerPhase::Up(_) => state.touch_up(finger),
                        PointerPhase::Cancel => state.touch_cancel(finger),
                        _ => {}
                    }
                }
                Forwarded::Left => {
                    state.pen_left(id);
                    if !hovered && !state.buttons_held() && state.pointer_window() == Some(id) {
                        state.pointer_motion(None);
                    }
                }
                _ => {}
            }
        }
        if touched {
            state.touch_frame();
        }
    }

    fn focus(&mut self, now: Instant) {
        let focused = self
            .keyboard_for(self.windows.focused())
            .map(|id| self.windows.blocked_by(id).unwrap_or(id));
        if self.server.state.keyboard_window() == focused {
            return;
        }
        let previous = self.server.state.keyboard_window();
        self.server.state.focus_keyboard(focused);
        if let Some(id) = focused {
            self.server.state.ping(id, now);
        }
        for id in previous.into_iter().chain(focused) {
            self.windows.push(Command::Configure(id));
        }
    }

    fn apply(&mut self, now: Instant) {
        let focused = self.server.state.keyboard_window();
        for command in self.windows.take_commands() {
            match command {
                Command::Configure(id) => {
                    if let Some(rect) = self.windows.rect(id) {
                        let position = rect.min - self.area.min;
                        self.server.state.place(
                            id,
                            (position.x.round() as i32, position.y.round() as i32).into(),
                        );
                    }
                    let size = self
                        .windows
                        .rect(id)
                        .map(|rect| (rect.width().round() as i32, rect.height().round() as i32))
                        .or_else(|| self.configured.get(&id).map(|configured| configured.size))
                        .or_else(|| {
                            self.fullscreen_area(id).map(|area| {
                                (area.width().round() as i32, area.height().round() as i32)
                            })
                        });
                    let own = !self.configured.contains_key(&id)
                        && size.is_some_and(|size| {
                            self.server.state.own_size(id) == Some(size.into())
                        });
                    let Some(size) = size.filter(|size| size.0 >= 1 && size.1 >= 1 && !own) else {
                        self.server
                            .state
                            .configure_sized(id, None, focused == Some(id));
                        continue;
                    };
                    let configured = Configured {
                        size,
                        activated: focused == Some(id),
                        fullscreen: self.server.state.fullscreen(id),
                    };
                    if self.configured.get(&id) == Some(&configured) {
                        continue;
                    }
                    self.configured.insert(id, configured);
                    self.server
                        .state
                        .configure(id, size.into(), configured.activated);
                }
                Command::Close(id) if self.server.state.responding(id) => {
                    self.server.state.close(id);
                    self.server.state.ping(id, now);
                }
                Command::Close(id) => self.force_close(id),
                Command::RestoreShortcuts(id) => {
                    self.server.state.restore_shortcuts(id);
                    self.forced = true;
                }
                Command::Launch(launch) => self.launch(&launch, now),
            }
        }
        let cursor = match self.server.state.cursor() {
            CursorImageStatus::Hidden => CursorIcon::None,
            CursorImageStatus::Named(icon) => cursor_icon(*icon),
            CursorImageStatus::Surface(_) => CursorIcon::Default,
        };
        let windows = self.windows;
        windows.set_cursor(cursor);
        self.reap();
    }
}

impl Compositor {
    pub fn replace_gpu(
        &mut self,
        device: wgpu::Device,
        queue: wgpu::Queue,
        format: wgpu::TextureFormat,
    ) {
        self.textures
            .borrow_mut()
            .set_gpu(Gpu::new(device, queue, format));
    }

    pub fn start(
        &mut self,
        device: wgpu::Device,
        queue: wgpu::Queue,
        format: wgpu::TextureFormat,
        waker: Waker,
    ) {
        self.textures
            .borrow_mut()
            .set_gpu(Gpu::new(device, queue, format));
        let dmabuf = self.textures.borrow().dmabuf_formats();
        if let Some((formats, render_node)) = dmabuf {
            let textures = self.textures.clone();
            self.server.state.enable_dmabuf(
                formats,
                render_node,
                Some(Box::new(move |dmabuf| {
                    textures.borrow_mut().import(dmabuf).is_some()
                })),
            );
            if let Some(render_node) = render_node {
                self.server.state.enable_explicit_sync(render_node);
            }
        }
        self.waker = Some(waker.clone());
        match self.server.watch(move || waker.wake()) {
            Ok(watch) => self.watch = Some(watch),
            Err(error) => eprintln!("be-wayland: could not watch the display: {error}"),
        }
    }

    pub fn join_loop<D: 'static>(
        &mut self,
        handle: &smithay::reexports::calloop::LoopHandle<'static, D>,
    ) {
        let Some(waker) = self.waker.clone() else {
            return;
        };
        self.watch = None;
        if let Err(error) = self.server.join_loop(handle, move || waker.wake()) {
            eprintln!("be-wayland: could not watch the display from the event loop: {error}");
        }
    }
}

impl Compositor {
    pub fn pace_to_flips(&mut self) {
        self.server.state.presentation.pace_to_flips();
    }

    pub fn presented(&mut self, screen: Rect, presented: Presented) {
        self.server.state.presentation.flipped(screen, presented);
    }

    pub fn awaits_flip(&self) -> bool {
        self.server.state.presentation.awaits_flip()
    }

    fn place_of(&self, id: WindowId) -> Place {
        let Some(rect) = self.windows.rect(id) else {
            return Place::Anywhere;
        };
        if self.monitors.is_empty() {
            return Place::Anywhere;
        }
        let area = |screen: &Rect| {
            let shared = screen.intersect(rect);
            shared.width().max(0.0) * shared.height().max(0.0)
        };
        self.monitors
            .iter()
            .map(|monitor| monitor.rect)
            .filter(|screen| area(screen) > 0.0)
            .max_by(|left, right| area(left).total_cmp(&area(right)))
            .map_or(Place::Nowhere, Place::On)
    }

    fn show_painted(&mut self, context: &Context) {
        let open = self.server.state.windows();
        let presentation = &mut self.server.state.presentation;
        for id in self.windows.take_painted() {
            presentation.painted(id);
        }
        presentation.forget_closed(&open);
        let places: Vec<_> = open.iter().map(|id| (*id, self.place_of(*id))).collect();
        let shown = self
            .server
            .state
            .presentation
            .take_shown(context.now(), |id| {
                places
                    .iter()
                    .find(|(held, _)| *held == id)
                    .map_or(Place::Anywhere, |(_, place)| *place)
            });
        for shown in shown {
            self.server.state.present(shown);
        }
    }

    pub fn before(&mut self, context: &Context, rect: Rect) {
        self.show_painted(context);
        self.receive(context);
        self.scale = context.pixels_per_point();
        self.area = match self.monitors.is_empty() {
            true => rect,
            false => self
                .monitors
                .iter()
                .map(|monitor| monitor.rect)
                .reduce(|all, rect| all.union(rect))
                .unwrap_or(rect),
        };
        let heads = self
            .shown_monitors()
            .into_iter()
            .map(|monitor| crate::heads::HeadSpec {
                id: monitor.id,
                name: monitor.name,
                rect: smithay::utils::Rectangle::new(
                    (
                        (monitor.rect.min.x - self.area.min.x).round() as i32,
                        (monitor.rect.min.y - self.area.min.y).round() as i32,
                    )
                        .into(),
                    (
                        monitor.rect.width().round() as i32,
                        monitor.rect.height().round() as i32,
                    )
                        .into(),
                ),
                scale: monitor.scale.ceil().max(1.0) as i32,
            })
            .collect();
        self.server.state.set_heads(heads);
        self.server
            .state
            .set_touchscreen(context.touchscreen_attached());
        self.publish();
        self.listed = self.windows.revision();
    }

    fn watch_idle(&mut self, context: &Context, document: &mut Document) {
        let now = context.now();
        let active = document.input_frames() != self.input_frames;
        self.input_frames = document.input_frames();
        let inhibited = !self.locked
            && self
                .server
                .state
                .inhibiting_windows()
                .into_iter()
                .any(|id| self.shown(id));
        if let Some(due) = self.server.state.idle.tick(now, active, inhibited) {
            context.request_repaint_after(due.saturating_duration_since(now));
        }
        let idle = self.server.state.idle.blanked();
        if self.windows.idle().get_untracked() != idle {
            let windows = self.windows;
            windows.set_idle(idle);
        }
    }

    fn shown(&self, id: WindowId) -> bool {
        self.windows.rect(id).is_some_and(|rect| {
            let visible = rect.intersect(self.area);
            visible.width() > 0.0 && visible.height() > 0.0
        })
    }

    pub fn after(&mut self, context: &Context, document: &mut Document) {
        let now = context.now();
        self.watch_idle(context, document);
        self.focus(now);
        self.input(now);
        self.apply(now);
        let typing = self
            .server
            .state
            .keyboard_window()
            .zip(self.server.state.text_inputs.asks());
        self.windows.set_typing(typing);
        self.server.flush();
        self.ping_unowned(context);
        self.watch_pings(context);
        if std::mem::take(&mut self.forced) || self.windows.revision() != self.listed {
            context.request_repaint();
        }
        if let Some(watch) = &self.watch {
            watch.resume();
        }
    }

    pub fn exiting(&mut self) {
        self.reap();
        for (pid, _) in &self.children {
            let _ = rustix::process::kill_process(*pid, rustix::process::Signal::KILL);
        }
    }
}

#[derive(Clone)]
struct Origin {
    launch: Option<u64>,
    command: Vec<String>,
}

fn command_line(pid: rustix::process::Pid) -> Vec<String> {
    let Ok(line) = std::fs::read(format!("/proc/{}/cmdline", pid.as_raw_nonzero())) else {
        return Vec::new();
    };
    line.split(|byte| *byte == 0)
        .filter(|argument| !argument.is_empty())
        .map(|argument| String::from_utf8_lossy(argument).into_owned())
        .collect()
}

struct Exit {
    pid: rustix::process::Pid,
    line: String,
    status: Option<i32>,
}

impl Exit {
    fn problem(&self) -> Option<String> {
        launch_problem(&self.line, self.status)
    }
}

fn launch_name(launch: &Launch) -> String {
    match launch.arguments.as_slice() {
        [shell, flag, line] if shell == "sh" && flag == "-c" => line.clone(),
        arguments => arguments.join(" "),
    }
}

pub(crate) fn launch_problem(line: &str, status: Option<i32>) -> Option<String> {
    match status {
        Some(127) => Some(format!("Could not run {line}: the command was not found")),
        Some(126) => Some(format!(
            "Could not run {line}: the command could not be started"
        )),
        _ => None,
    }
}

fn parent(pid: rustix::process::Pid) -> Option<rustix::process::Pid> {
    let stat = std::fs::read_to_string(format!("/proc/{}/stat", pid.as_raw_nonzero())).ok()?;
    let (_, fields) = stat.rsplit_once(')')?;
    let parent = fields.split_whitespace().nth(1)?.parse().ok()?;
    rustix::process::Pid::from_raw(parent).filter(|parent| !parent.is_init())
}

fn button_code(button: PointerButton) -> u32 {
    match button {
        PointerButton::Primary => BUTTON_LEFT,
        PointerButton::Secondary => BUTTON_RIGHT,
        PointerButton::Middle => BUTTON_MIDDLE,
        PointerButton::Back => BUTTON_SIDE,
        PointerButton::Forward => BUTTON_EXTRA,
        PointerButton::Other(code) => u32::from(code),
    }
}

fn cursor_icon(icon: smithay::input::pointer::CursorIcon) -> CursorIcon {
    CursorIcon::from_name(icon.name()).unwrap_or_default()
}

mod clipboard;
mod unowned;

#[cfg(test)]
mod tests;
