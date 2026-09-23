use std::process::{Child, Command as Process, Stdio};

use beui::reactive::{build, view, with_reactive_scope};
use beui::{
    App, Color32, Context, CursorIcon, Document, Event, PointerButton, Pos2, Rect, Setup, Waker,
};
use smithay::input::pointer::CursorImageStatus;

use crate::clients::{Clients, Command};
use crate::render::{Gpu, Textures, WindowDraw};
use crate::server::{Server, Watch};
use crate::state::{ServerEvent, WindowId};
use crate::ui::Workspace;

const BUTTON_LEFT: u32 = 0x110;
const BUTTON_RIGHT: u32 = 0x111;
const BUTTON_MIDDLE: u32 = 0x112;
const BUTTON_SIDE: u32 = 0x113;
const BUTTON_EXTRA: u32 = 0x114;

pub struct Compositor {
    server: Server,
    clients: Clients,
    document: Document,
    textures: Textures,
    watch: Option<Watch>,
    waker: Option<Waker>,
    launches: Vec<String>,
    children: Vec<Child>,
    grab: Option<WindowId>,
    held: Vec<u32>,
    configured: std::collections::HashMap<WindowId, ((i32, i32), bool)>,
}

impl Compositor {
    pub fn new(server: Server, launches: Vec<String>) -> Self {
        let socket = server
            .socket_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        let clients = Clients::new(socket);
        let document = build(clone_clients(&clients));
        Self {
            server,
            clients,
            document,
            textures: Textures::default(),
            watch: None,
            waker: None,
            launches,
            children: Vec::new(),
            grab: None,
            held: Vec::new(),
            configured: std::collections::HashMap::new(),
        }
    }

    pub fn server(&mut self) -> &mut Server {
        &mut self.server
    }

    pub fn clients(&self) -> &Clients {
        &self.clients
    }

    pub fn document(&self) -> &Document {
        &self.document
    }

    fn launch(&mut self, line: &str) {
        let mut process = Process::new("sh");
        process
            .arg("-c")
            .arg(line)
            .env("WAYLAND_DISPLAY", self.clients.socket())
            .env_remove("DISPLAY")
            .env("GDK_BACKEND", "wayland")
            .env("QT_QPA_PLATFORM", "wayland")
            .env("SDL_VIDEODRIVER", "wayland")
            .env("MOZ_ENABLE_WAYLAND", "1")
            .env("ELECTRON_OZONE_PLATFORM_HINT", "wayland")
            .stdin(Stdio::null());
        match process.spawn() {
            Ok(child) => self.children.push(child),
            Err(error) => eprintln!("be-compositor: could not run {line}: {error}"),
        }
    }

    fn receive(&mut self) {
        self.server.dispatch();
        let events = self.server.state.take_events();
        for surface in self.server.state.take_committed() {
            self.textures.upload(&surface);
        }
        self.textures.prune();
        let clients = self.clients.clone();
        let mut redraw = Vec::new();
        with_reactive_scope(&mut self.document, || {
            for event in events {
                match event {
                    ServerEvent::Opened(id) => clients.open(id),
                    ServerEvent::Closed(id) => clients.close(id),
                    ServerEvent::Titled(id, title) => clients.retitle(id, title),
                    ServerEvent::Committed(id) => {
                        if !redraw.contains(&id) {
                            redraw.push(id);
                        }
                    }
                }
            }
        });
        let windows = self.server.state.windows();
        self.configured.retain(|id, _| windows.contains(id));
        if redraw.is_empty() {
            return;
        }
        let drawings: Vec<_> = redraw
            .into_iter()
            .map(|id| (id, self.drawing(id)))
            .collect();
        let clients = self.clients.clone();
        with_reactive_scope(&mut self.document, || {
            for (id, drawing) in drawings {
                clients.draw(id, drawing);
            }
        });
    }

    fn drawing(&self, id: WindowId) -> Option<beui::Drawing> {
        let gpu = self.textures.gpu()?;
        let signals = self.clients.signals(id)?;
        let layers = self
            .server
            .state
            .layers(id)
            .into_iter()
            .filter_map(|layer| Some((self.textures.get(&layer.surface)?, layer)))
            .collect();
        Some(WindowDraw::drawing(
            gpu,
            layers,
            signals.painted.clone(),
            self.waker.clone(),
        ))
    }

    fn keyboard(&mut self, context: &Context) {
        let Some(id) = self.clients.focused() else {
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
                        let Some(id) = self.grab.or(self.clients.hovered()) else {
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
                Event::Scroll(delta) if self.grab.or(self.clients.hovered()).is_some() => {
                    self.server
                        .state
                        .pointer_axis((-f64::from(delta.x), -f64::from(delta.y)));
                }
                _ => {}
            }
        }
    }

    fn move_pointer(&mut self, position: Pos2) {
        let target = self.grab.or(self.clients.hovered());
        let local = target.and_then(|id| {
            let rect = self.clients.rect(id)?;
            let local = position - rect.min;
            Some((id, (f64::from(local.x), f64::from(local.y)).into()))
        });
        if local.is_none() && self.server.state.pointer_window().is_none() {
            return;
        }
        self.server.state.pointer_motion(local);
    }

    fn apply(&mut self) {
        let focused = self.clients.focused();
        if self.server.state.keyboard_window() != focused {
            let previous = self.server.state.keyboard_window();
            self.server.state.focus_keyboard(focused);
            for id in previous.into_iter().chain(focused) {
                self.clients.push(Command::Configure(id));
            }
        }
        for command in self.clients.take_commands() {
            match command {
                Command::Configure(id) => {
                    let Some(rect) = self.clients.rect(id) else {
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
                Command::Launch(line) => self.launch(&line),
            }
        }
        for id in self.clients.take_painted() {
            self.server.state.send_frames(id);
        }
        let cursor = match self.server.state.cursor() {
            CursorImageStatus::Hidden => CursorIcon::None,
            CursorImageStatus::Named(icon) => cursor_icon(*icon),
            CursorImageStatus::Surface(_) => CursorIcon::Default,
        };
        let clients = self.clients.clone();
        with_reactive_scope(&mut self.document, || clients.set_cursor(cursor));
        self.children
            .retain_mut(|child| matches!(child.try_wait(), Ok(None)));
    }
}

fn clone_clients(clients: &Clients) -> impl FnOnce() -> beui::NodeId + use<> {
    let clients = clients.clone();
    move || {
        view! {
            <Workspace clients />
        }
    }
}

impl App for Compositor {
    fn setup(&mut self, setup: &Setup) {
        self.textures.set_gpu(Gpu::new(
            setup.device.clone(),
            setup.queue.clone(),
            setup.format,
        ));
        self.waker = Some(setup.waker.clone());
        let waker = setup.waker.clone();
        match self.server.watch(move || waker.wake()) {
            Ok(watch) => self.watch = Some(watch),
            Err(error) => eprintln!("be-compositor: could not watch the display: {error}"),
        }
        for line in std::mem::take(&mut self.launches) {
            self.launch(&line);
        }
    }

    fn update(&mut self, context: &Context, rect: Rect) {
        self.receive();
        let scale = context.pixels_per_point().ceil().max(1.0) as i32;
        let size = (rect.width().round() as i32, rect.height().round() as i32);
        self.server.state.set_scale(scale, size.into());
        self.keyboard(context);
        self.document.show(context, rect);
        self.pointer(context);
        self.apply();
        self.server.flush();
        if let Some(watch) = &self.watch {
            watch.resume();
        }
    }

    fn clear_color(&self) -> Color32 {
        self.document.theme().background
    }

    fn exiting(&mut self) {
        for child in &mut self.children {
            let _ = child.kill();
        }
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
