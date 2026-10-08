use std::cell::RefCell;
use std::process::{Command as Process, Stdio};
use std::rc::Rc;
use std::sync::mpsc::{Receiver, Sender, channel};

use beui::reactive::with_reactive_scope;
use beui::{Context, CursorIcon, Document, Event, PointerButton, Pos2, Rect, Vec2, Waker};
use smithay::backend::renderer::utils::with_renderer_surface_state;
use smithay::input::pointer::{CursorImageStatus, CursorImageSurfaceData};
use smithay::wayland::compositor::with_states;

use crate::render::{Gpu, Textures, WindowDraw};
use crate::server::{Server, Watch};
use crate::state::{KeyboardConfig, ServerEvent, WindowId};
use crate::windows::{Command, Launch, WindowInfo, Windows};

const BUTTON_LEFT: u32 = 0x110;
const BUTTON_RIGHT: u32 = 0x111;
const BUTTON_MIDDLE: u32 = 0x112;
const BUTTON_SIDE: u32 = 0x113;
const BUTTON_EXTRA: u32 = 0x114;

pub struct CursorImage {
    pub texture: wgpu::Texture,
    pub size: Vec2,
    pub hotspot: Vec2,
    pub opaque: bool,
}

pub struct Compositor {
    server: Server,
    windows: Windows,
    textures: Rc<RefCell<Textures>>,
    watch: Option<Watch>,
    waker: Option<Waker>,
    children: Vec<rustix::process::Pid>,
    exited: (Sender<Exit>, Receiver<Exit>),
    on_failure: Option<Box<dyn Fn(String)>>,
    grab: Option<WindowId>,
    held: Vec<u32>,
    configured: std::collections::HashMap<WindowId, ((i32, i32), bool)>,
}

impl Compositor {
    pub fn new(server: Server, windows: Windows) -> Self {
        Self {
            server,
            windows,
            textures: Rc::new(RefCell::new(Textures::default())),
            watch: None,
            waker: None,
            children: Vec::new(),
            exited: channel(),
            on_failure: None,
            grab: None,
            held: Vec::new(),
            configured: std::collections::HashMap::new(),
        }
    }

    pub fn server(&mut self) -> &mut Server {
        &mut self.server
    }

    pub fn set_keyboard(&mut self, keyboard: &KeyboardConfig) -> bool {
        self.server.state.set_keyboard(keyboard)
    }

    pub fn windows(&self) -> Windows {
        self.windows.clone()
    }

    pub fn socket(&self) -> Option<String> {
        self.server
            .socket_name()
            .map(|name| name.to_string_lossy().into_owned())
    }

    fn launch(&mut self, launch: &Launch) {
        let Some((program, arguments)) = launch.arguments.split_first() else {
            return;
        };
        let mut process = Process::new(program);
        if let Some(dir) = &launch.working_dir {
            process.current_dir(dir);
        }
        process
            .args(arguments)
            .env("WAYLAND_DISPLAY", self.socket().unwrap_or_default())
            .env_remove("DISPLAY")
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
        self.children.push(pid);
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
            self.children.retain(|pid| *pid != exit.pid);
            if let Some(problem) = exit.problem() {
                self.fail(problem);
            }
        }
    }

    fn receive(&mut self, document: &mut Document) {
        self.server.dispatch();
        let events = self.server.state.take_events();
        for surface in self.server.state.take_committed() {
            self.textures.borrow_mut().upload(&surface);
        }
        self.textures.borrow_mut().prune();
        let mut redraw = Vec::new();
        let mut changed = false;
        let windows = self.windows.clone();
        with_reactive_scope(document, || {
            for event in events {
                match event {
                    ServerEvent::Opened(id) => windows.open(id),
                    ServerEvent::Closed(id) => {
                        windows.forget(id);
                        changed = true;
                    }
                    ServerEvent::Titled(..) | ServerEvent::Changed(_) => changed = true,
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
        });
        let open = self.server.state.windows();
        self.configured.retain(|id, _| open.contains(id));
        if changed {
            let list = self.list();
            let windows = self.windows.clone();
            with_reactive_scope(document, || windows.set_list(list));
        }
        if !redraw.is_empty() {
            self.redraw(redraw, document);
        }
    }

    fn list(&self) -> Vec<WindowInfo> {
        let state = &self.server.state;
        state
            .windows()
            .into_iter()
            .filter_map(|id| {
                Some(WindowInfo {
                    id,
                    size: state.mapped_size(id)?,
                    title: state.title(id).unwrap_or_default(),
                    app_id: state.app_id(id).unwrap_or_default(),
                    parent: state.parent(id),
                })
            })
            .collect()
    }

    fn redraw(&mut self, windows: Vec<WindowId>, document: &mut Document) {
        let drawings: Vec<_> = windows
            .into_iter()
            .map(|id| (id, self.drawing(id)))
            .collect();
        let store = self.windows.clone();
        with_reactive_scope(document, || {
            for (id, drawing) in drawings {
                store.draw(id, drawing);
            }
        });
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
        })
    }

    fn drawing(&self, id: WindowId) -> Option<beui::Drawing> {
        let textures = self.textures.borrow();
        let gpu = textures.gpu()?;
        let signals = self.windows.signals(id)?;
        let layers = self
            .server
            .state
            .layers(id)
            .into_iter()
            .filter_map(|layer| Some((textures.get(&layer.surface)?, layer)))
            .collect();
        Some(WindowDraw::drawing(
            gpu,
            layers,
            signals.painted.clone(),
            self.waker.clone(),
        ))
    }

    fn keyboard(&mut self, context: &Context) {
        let Some(id) = self.windows.focused() else {
            return;
        };
        if self.server.state.keyboard_window() != Some(id) {
            return;
        }
        let keys: Vec<(u32, bool)> = context.input(|input| {
            input
                .events
                .iter()
                .filter_map(|event| match event {
                    Event::PhysicalKey { code, pressed } => Some((*code, *pressed)),
                    _ => None,
                })
                .collect()
        });
        for (code, pressed) in keys {
            self.server.state.key(code, pressed);
        }
        context.retain_events(|event| {
            !matches!(
                event,
                Event::Key { .. } | Event::Text(_) | Event::Ime(_) | Event::PhysicalKey { .. }
            )
        });
    }

    fn pointer(&mut self, context: &Context) {
        let events = context.input(|input| input.events.clone());
        for event in events {
            match event {
                Event::PointerMoved(position) => self.move_pointer(position),
                Event::PointerGone if self.grab.is_none() => {
                    if self.server.state.pointer_window().is_some() {
                        self.server.state.pointer_motion(None);
                    }
                }
                Event::PointerButton {
                    pos,
                    button,
                    pressed,
                    ..
                } => {
                    self.move_pointer(pos);
                    let code = button_code(button);
                    if pressed {
                        let Some(id) = self.grab.or(self.windows.hovered()) else {
                            self.server.state.dismiss_popups();
                            continue;
                        };
                        self.grab = Some(id);
                        if !self.held.contains(&code) {
                            self.held.push(code);
                        }
                        self.server.state.pointer_button(code, true);
                    } else if self.held.contains(&code) {
                        self.held.retain(|held| *held != code);
                        self.server.state.pointer_button(code, false);
                        if self.held.is_empty() {
                            self.grab = None;
                            self.move_pointer(pos);
                        }
                    }
                }
                Event::Scroll(delta) if self.grab.or(self.windows.hovered()).is_some() => {
                    self.server
                        .state
                        .pointer_axis((-f64::from(delta.x), -f64::from(delta.y)));
                }
                _ => {}
            }
        }
    }

    fn move_pointer(&mut self, position: Pos2) {
        let target = self.grab.or(self.windows.hovered());
        let local = target.and_then(|id| {
            let rect = self.windows.rect(id)?;
            let local = position - rect.min;
            Some((id, (f64::from(local.x), f64::from(local.y)).into()))
        });
        if local.is_none() && self.server.state.pointer_window().is_none() {
            return;
        }
        self.server.state.pointer_motion(local);
    }

    fn apply(&mut self, document: &mut Document) {
        let focused = self.windows.focused();
        if self.server.state.keyboard_window() != focused {
            let previous = self.server.state.keyboard_window();
            self.server.state.focus_keyboard(focused);
            for id in previous.into_iter().chain(focused) {
                self.windows.push(Command::Configure(id));
            }
        }
        for command in self.windows.take_commands() {
            match command {
                Command::Configure(id) => {
                    let Some(rect) = self.windows.rect(id) else {
                        continue;
                    };
                    let size = (rect.width().round() as i32, rect.height().round() as i32);
                    if size.0 < 1 || size.1 < 1 {
                        continue;
                    }
                    let activated = focused == Some(id);
                    if self.configured.get(&id) == Some(&(size, activated)) {
                        continue;
                    }
                    self.configured.insert(id, (size, activated));
                    self.server.state.configure(id, size.into(), activated);
                }
                Command::Close(id) => self.server.state.close(id),
                Command::Launch(launch) => self.launch(&launch),
            }
        }
        for id in self.windows.take_painted() {
            self.server.state.send_frames(id);
        }
        let cursor = match self.server.state.cursor() {
            CursorImageStatus::Hidden => CursorIcon::None,
            CursorImageStatus::Named(icon) => cursor_icon(*icon),
            CursorImageStatus::Surface(_) => CursorIcon::Default,
        };
        let windows = self.windows.clone();
        with_reactive_scope(document, || windows.set_cursor(cursor));
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
}

impl Compositor {
    pub fn before(&mut self, context: &Context, rect: Rect, document: &mut Document) {
        self.receive(document);
        let scale = context.pixels_per_point().ceil().max(1.0) as i32;
        let size = (rect.width().round() as i32, rect.height().round() as i32);
        self.server.state.set_scale(scale, size.into());
        self.keyboard(context);
    }

    pub fn after(&mut self, context: &Context, document: &mut Document) {
        self.pointer(context);
        self.apply(document);
        self.server.flush();
        if let Some(watch) = &self.watch {
            watch.resume();
        }
    }

    pub fn exiting(&mut self) {
        self.reap();
        for pid in &self.children {
            let _ = rustix::process::kill_process(*pid, rustix::process::Signal::KILL);
        }
    }
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

fn button_code(button: PointerButton) -> u32 {
    match button {
        PointerButton::Primary => BUTTON_LEFT,
        PointerButton::Secondary => BUTTON_RIGHT,
        PointerButton::Middle => BUTTON_MIDDLE,
        PointerButton::Back => BUTTON_SIDE,
        PointerButton::Forward => BUTTON_EXTRA,
    }
}

fn cursor_icon(icon: smithay::input::pointer::CursorIcon) -> CursorIcon {
    use smithay::input::pointer::CursorIcon as Named;
    match icon {
        Named::Crosshair => CursorIcon::Crosshair,
        Named::Grab => CursorIcon::Grab,
        Named::Grabbing => CursorIcon::Grabbing,
        Named::NotAllowed | Named::NoDrop => CursorIcon::NotAllowed,
        Named::Pointer => CursorIcon::PointingHand,
        Named::EwResize | Named::EResize | Named::WResize | Named::ColResize => {
            CursorIcon::ResizeHorizontal
        }
        Named::NsResize | Named::NResize | Named::SResize | Named::RowResize => {
            CursorIcon::ResizeVertical
        }
        Named::NeswResize | Named::NeResize | Named::SwResize => CursorIcon::ResizeNeSw,
        Named::NwseResize | Named::NwResize | Named::SeResize => CursorIcon::ResizeNwSe,
        Named::Text | Named::VerticalText => CursorIcon::Text,
        Named::Wait => CursorIcon::Wait,
        Named::Move | Named::AllScroll => CursorIcon::Move,
        Named::Progress => CursorIcon::Progress,
        Named::Help => CursorIcon::Help,
        Named::Alias => CursorIcon::Alias,
        _ => CursorIcon::Default,
    }
}

#[cfg(test)]
mod tests;
