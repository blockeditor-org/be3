use std::ffi::OsStr;
use std::fmt;
use std::os::fd::OwnedFd;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use smithay::delegate_xwayland_shell;
use smithay::desktop::Window;
use smithay::desktop::utils::{send_frames_surface_tree, with_surfaces_surface_tree};
use smithay::reexports::calloop::{LoopHandle, RegistrationToken};
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::reexports::wayland_server::{Client, DisplayHandle};
use smithay::utils::{Logical, Point, Rectangle, Size, Transform};
use smithay::wayland::compositor::send_surface_state;
use smithay::wayland::selection::SelectionTarget;
use smithay::wayland::xwayland_shell::{XWaylandShellHandler, XWaylandShellState};
use smithay::xwayland::xwm::{Reorder, ResizeEdge, WmWindowProperty, X11Window, XwmId};
use smithay::xwayland::{X11Surface, X11Wm, XWayland, XWaylandEvent, XwmHandler};

use crate::state::{ClientWindow, Grab, Layer, ServerEvent, State, WindowId, collect_layers};

const MAX_RESTARTS: u32 = 3;
const PROGRAM: &str = "Xwayland";
const SOCKET_DIR: &str = "/tmp/.X11-unix";

#[derive(Debug)]
pub(crate) enum StartError {
    NotInstalled,
    SocketDirNotADirectory(PathBuf),
    SocketDirUnmade(PathBuf, std::io::Error),
    SocketDirUnwritable(PathBuf, std::io::Error),
    NoFreeDisplay(Option<u32>),
    Other(std::io::Error),
}

impl fmt::Display for StartError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotInstalled => write!(
                f,
                "{PROGRAM} is not installed (it is in the xwayland package), or is not on PATH"
            ),
            Self::SocketDirNotADirectory(dir) => write!(
                f,
                "{}, where X servers put their sockets, is not a directory",
                dir.display()
            ),
            Self::SocketDirUnmade(dir, error) => write!(
                f,
                "{}, where X servers put their sockets, is missing and could not be made: {error}",
                dir.display()
            ),
            Self::SocketDirUnwritable(dir, error) => write!(
                f,
                "{}, where X servers put their sockets, cannot be written to: {error}",
                dir.display()
            ),
            Self::NoFreeDisplay(Some(display)) => write!(
                f,
                "display :{display} is taken (its lock file /tmp/.X{display}-lock is another X server's, or its socket could not be made)"
            ),
            Self::NoFreeDisplay(None) => write!(
                f,
                "no display from :0 to :32 is free (each has another X server's lock file in /tmp, or its socket could not be made)"
            ),
            Self::Other(error) => write!(f, "{error}"),
        }
    }
}

pub(crate) fn find_program(name: &str, path: &OsStr) -> Option<PathBuf> {
    std::env::split_paths(path)
        .map(|dir| dir.join(name))
        .find(|candidate| {
            candidate
                .metadata()
                .is_ok_and(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
        })
}

pub(crate) fn prepare_socket_dir(dir: &Path) -> Result<(), StartError> {
    match std::fs::symlink_metadata(dir) {
        Ok(meta) if !meta.is_dir() => {
            return Err(StartError::SocketDirNotADirectory(dir.to_owned()));
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            match std::fs::create_dir(dir) {
                Ok(()) => std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o1777))
                    .map_err(|error| StartError::SocketDirUnmade(dir.to_owned(), error))?,
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(StartError::SocketDirUnmade(dir.to_owned(), error)),
            }
        }
        Err(error) => return Err(StartError::SocketDirUnwritable(dir.to_owned(), error)),
    }
    rustix::fs::access(dir, rustix::fs::Access::WRITE_OK | rustix::fs::Access::EXEC_OK)
        .map_err(|error| StartError::SocketDirUnwritable(dir.to_owned(), error.into()))
}

fn preflight(path: Option<&OsStr>, socket_dir: &Path) -> Result<(), StartError> {
    if find_program(PROGRAM, path.unwrap_or_default()).is_none() {
        return Err(StartError::NotInstalled);
    }
    prepare_socket_dir(socket_dir)
}
const MAX_TRANSIENTS: usize = 32;

pub(crate) struct Overlay {
    surface: X11Surface,
    owner: WindowId,
}

pub(crate) struct XWaylandState {
    loop_handle: LoopHandle<'static, State>,
    shell: XWaylandShellState,
    wm: Option<X11Wm>,
    source: Option<RegistrationToken>,
    display: Option<u32>,
    stopped: bool,
    restarts: u32,
    gave_up: bool,
    failure: Option<String>,
    overlays: Vec<Overlay>,
    clipboard: Option<Vec<String>>,
    primary: Option<Vec<String>>,
    #[cfg(test)]
    pub(crate) asked: Vec<(SelectionTarget, String, OwnedFd)>,
}

impl XWaylandState {
    pub(crate) fn new(handle: &DisplayHandle, loop_handle: LoopHandle<'static, State>) -> Self {
        Self {
            loop_handle,
            shell: XWaylandShellState::new::<State>(handle),
            wm: None,
            source: None,
            display: None,
            stopped: false,
            restarts: 0,
            gave_up: false,
            failure: None,
            overlays: Vec::new(),
            clipboard: None,
            primary: None,
            #[cfg(test)]
            asked: Vec::new(),
        }
    }

    pub(crate) fn display(&self) -> Option<u32> {
        self.display
    }

    #[cfg(test)]
    pub(crate) fn offered(&self, target: SelectionTarget) -> Option<&[String]> {
        match target {
            SelectionTarget::Clipboard => self.clipboard.as_deref(),
            SelectionTarget::Primary => self.primary.as_deref(),
        }
    }

    fn offer_selection(&mut self, target: SelectionTarget) {
        let mime_types = match target {
            SelectionTarget::Clipboard => self.clipboard.clone(),
            SelectionTarget::Primary => self.primary.clone(),
        };
        if let Some(wm) = self.wm.as_mut()
            && let Err(error) = wm.new_selection(target, mime_types)
        {
            eprintln!("be-wayland: Xwayland was not told of the {target:?} selection: {error}");
        }
    }
}

impl State {
    pub(crate) fn start_xwayland(&mut self) -> Option<u32> {
        if let Some(display) = self.xwayland.display {
            return Some(display);
        }
        if self.xwayland.gave_up {
            return None;
        }
        match self.spawn_xwayland() {
            Ok(display) => {
                eprintln!("be-wayland: X11 programs run on Xwayland at :{display}");
                self.xwayland.failure = None;
                Some(display)
            }
            Err(error) => {
                let message = error.to_string();
                if self.xwayland.failure.as_ref() != Some(&message) {
                    eprintln!(
                        "be-wayland: Xwayland did not start, so X11 programs cannot run until it does: {message}"
                    );
                    self.xwayland.failure = Some(message);
                }
                None
            }
        }
    }

    fn spawn_xwayland(&mut self) -> Result<u32, StartError> {
        preflight(
            std::env::var_os("PATH").as_deref(),
            Path::new(SOCKET_DIR),
        )?;
        let wanted = self.xwayland.display;
        let (xwayland, client) = XWayland::spawn(
            &self.handle,
            wanted,
            std::iter::empty::<(String, String)>(),
            true,
            Stdio::null(),
            Stdio::null(),
            |_| (),
        )
        .map_err(|error| match error.kind() {
            std::io::ErrorKind::AddrInUse => StartError::NoFreeDisplay(wanted),
            std::io::ErrorKind::NotFound => StartError::NotInstalled,
            _ => StartError::Other(error),
        })?;
        let display = xwayland.display_number();
        let source = self
            .xwayland
            .loop_handle
            .insert_source(xwayland, move |event, (), state: &mut State| match event {
                XWaylandEvent::Ready { x11_socket, .. } => {
                    state.xwayland_ready(x11_socket, client.clone());
                }
                XWaylandEvent::Error => {
                    eprintln!("be-wayland: Xwayland exited while it was starting");
                    state.xwayland.stopped = true;
                }
            })
            .map_err(|error| StartError::Other(std::io::Error::other(error.error)))?;
        self.xwayland.source = Some(source);
        self.xwayland.display = Some(display);
        Ok(display)
    }

    fn xwayland_ready(&mut self, connection: UnixStream, client: Client) {
        let handle = self.xwayland.loop_handle.clone();
        match X11Wm::start_wm(handle, connection, client) {
            Ok(wm) => {
                self.xwayland.wm = Some(wm);
                for target in [SelectionTarget::Clipboard, SelectionTarget::Primary] {
                    self.xwayland.offer_selection(target);
                }
            }
            Err(error) => {
                eprintln!("be-wayland: the X11 window manager did not start: {error}");
                self.xwayland.stopped = true;
            }
        }
    }

    pub(crate) fn settle_xwayland(&mut self) {
        if !std::mem::take(&mut self.xwayland.stopped) {
            return;
        }
        self.xwayland.wm = None;
        if let Some(source) = self.xwayland.source.take() {
            self.xwayland.loop_handle.remove(source);
        }
        self.xwayland.overlays.clear();
        self.x11_cleared(SelectionTarget::Clipboard);
        self.x11_cleared(SelectionTarget::Primary);
        let closed: Vec<WindowId> = self
            .windows
            .iter()
            .filter(|window| window.window.is_x11())
            .map(|window| window.id)
            .collect();
        for id in closed {
            self.remove_window(id);
        }
        if self.xwayland.restarts >= MAX_RESTARTS {
            eprintln!("be-wayland: Xwayland stopped too often, so X11 programs cannot run");
            self.xwayland.display = None;
            self.xwayland.gave_up = true;
            return;
        }
        self.xwayland.restarts += 1;
        if let Err(error) = self.spawn_xwayland() {
            eprintln!("be-wayland: Xwayland did not start again: {error}");
            self.xwayland.display = None;
            self.xwayland.failure = Some(error.to_string());
        }
    }

    pub(crate) fn announce_x11_selection(
        &mut self,
        target: SelectionTarget,
        mime_types: Option<Vec<String>>,
    ) {
        match target {
            SelectionTarget::Clipboard => self.xwayland.clipboard = mime_types.clone(),
            SelectionTarget::Primary => self.xwayland.primary = mime_types.clone(),
        }
        self.xwayland.offer_selection(target);
    }

    pub(crate) fn read_x11_selection(
        &mut self,
        target: SelectionTarget,
        mime_type: String,
        fd: OwnedFd,
    ) -> bool {
        #[cfg(test)]
        if self.xwayland.wm.is_none() {
            self.xwayland.asked.push((target, mime_type, fd));
            return true;
        }
        let loop_handle = self.xwayland.loop_handle.clone();
        let Some(wm) = self.xwayland.wm.as_mut() else {
            return false;
        };
        match wm.send_selection(target, mime_type, fd, loop_handle) {
            Ok(()) => true,
            Err(error) => {
                eprintln!("be-wayland: the X11 {target:?} selection was not read: {error}");
                false
            }
        }
    }

    pub(crate) fn x11_window(&self, window: X11Window) -> Option<WindowId> {
        self.windows
            .iter()
            .find(|client| {
                client
                    .window
                    .x11_surface()
                    .is_some_and(|surface| surface.window_id() == window)
            })
            .map(|client| client.id)
    }

    fn x11_window_of(&self, surface: &X11Surface) -> Option<WindowId> {
        self.windows
            .iter()
            .find(|client| client.window.x11_surface() == Some(surface))
            .map(|client| client.id)
    }

    fn x11_surface(&self, id: WindowId) -> Option<X11Surface> {
        self.find(id)?.window.x11_surface().cloned()
    }

    fn open_x11(&mut self, surface: X11Surface) {
        if self.x11_window_of(&surface).is_some() {
            return;
        }
        let id = WindowId(self.next_window);
        self.next_window += 1;
        let title = surface.title();
        self.windows.push(ClientWindow {
            id,
            window: Window::new_x11_window(surface),
            title: title.clone(),
            fullscreen: false,
            maximized: false,
            position: Point::default(),
            chosen: None,
            entered: None,
        });
        self.events.push(ServerEvent::Opened(id));
        self.events.push(ServerEvent::Titled(id, title));
    }

    pub(crate) fn configure_x11(
        &mut self,
        id: WindowId,
        size: Option<Size<i32, Logical>>,
        activated: bool,
    ) {
        let Some(window) = self.find(id) else {
            return;
        };
        let Some(surface) = window.window.x11_surface() else {
            return;
        };
        let _ = surface.set_activated(activated);
        let _ = surface.set_fullscreen(window.fullscreen);
        let _ = surface.set_maximized(window.maximized);
        let size = size.unwrap_or(surface.geometry().size);
        let _ = surface.configure(Rectangle::new(window.position, size));
    }

    pub(crate) fn place_x11(&mut self, id: WindowId) {
        let Some(window) = self.find(id) else {
            return;
        };
        let Some(surface) = window.window.x11_surface() else {
            return;
        };
        let geometry = surface.geometry();
        if geometry.loc != window.position {
            let _ = surface.configure(Rectangle::new(window.position, geometry.size));
        }
    }

    pub(crate) fn raise_x11(&mut self, id: WindowId) {
        let Some(surface) = self.x11_surface(id) else {
            return;
        };
        if let Some(wm) = self.xwayland.wm.as_mut() {
            let _ = wm.raise_window(&surface);
        }
    }

    fn close_x11(&mut self, surface: &X11Surface) {
        self.xwayland
            .overlays
            .retain(|overlay| overlay.surface != *surface);
        if let Some(id) = self.x11_window_of(surface) {
            self.remove_window(id);
        }
    }

    pub(crate) fn remove_overlays(&mut self, owner: WindowId) {
        self.xwayland
            .overlays
            .retain(|overlay| overlay.owner != owner);
    }

    fn overlay_owner(&self, surface: &X11Surface) -> Option<WindowId> {
        let mut transient = surface.is_transient_for();
        for _ in 0..MAX_TRANSIENTS {
            let Some(parent) = transient else {
                break;
            };
            if let Some(id) = self.x11_window(parent) {
                return Some(id);
            }
            let overlay = self
                .xwayland
                .overlays
                .iter()
                .find(|overlay| overlay.surface.window_id() == parent);
            match overlay {
                Some(overlay) => return Some(overlay.owner),
                None => transient = None,
            }
        }
        let x11 = |id: Option<WindowId>| id.filter(|id| self.x11_surface(*id).is_some());
        let focused = [x11(self.pointer_window), x11(self.keyboard_window)];
        let location = surface.geometry().loc;
        let containing = focused
            .into_iter()
            .flatten()
            .chain(self.windows.iter().map(|window| window.id))
            .find(|id| {
                self.x11_surface(*id)
                    .is_some_and(|toplevel| toplevel.geometry().contains(location))
            });
        containing.or_else(|| focused.into_iter().flatten().next())
    }

    fn overlay_of(&self, surface: &WlSurface) -> Option<&Overlay> {
        self.xwayland
            .overlays
            .iter()
            .find(|overlay| overlay.surface.wl_surface().as_ref() == Some(surface))
    }

    fn redraw(&mut self, id: WindowId) {
        if !self.events.contains(&ServerEvent::Committed(id)) {
            self.events.push(ServerEvent::Committed(id));
        }
    }

    pub(crate) fn committed_overlay(&mut self, root: &WlSurface) -> bool {
        let Some(owner) = self.overlay_of(root).map(|overlay| overlay.owner) else {
            return false;
        };
        let output = self.output_of(owner);
        let scale = self.scale_of(owner);
        with_surfaces_surface_tree(root, |surface, data| {
            if let Some(output) = &output {
                output.enter(surface);
            }
            send_surface_state(surface, data, scale, Transform::Normal);
        });
        self.redraw(owner);
        true
    }

    fn overlay_origin(&self, overlay: &Overlay) -> Option<Point<i32, Logical>> {
        let toplevel = self.x11_surface(overlay.owner)?;
        Some(overlay.surface.geometry().loc - toplevel.geometry().loc)
    }

    pub(crate) fn overlay_layers(&self, id: WindowId, layers: &mut Vec<Layer>) {
        for overlay in self.xwayland.overlays.iter().filter(|o| o.owner == id) {
            if let (Some(surface), Some(origin)) =
                (overlay.surface.wl_surface(), self.overlay_origin(overlay))
            {
                collect_layers(&surface, origin, true, layers);
            }
        }
    }

    pub(crate) fn overlay_under(
        &self,
        id: WindowId,
        location: Point<f64, Logical>,
    ) -> Option<(WlSurface, Point<f64, Logical>)> {
        self.xwayland
            .overlays
            .iter()
            .rev()
            .filter(|overlay| overlay.owner == id)
            .find_map(|overlay| {
                let origin = self.overlay_origin(overlay)?;
                let rect = Rectangle::new(origin, overlay.surface.geometry().size);
                if !rect.to_f64().contains(location) {
                    return None;
                }
                Some((overlay.surface.wl_surface()?, origin.to_f64()))
            })
    }

    pub(crate) fn send_overlay_frames(&self, id: WindowId, time: Duration) {
        for overlay in self.xwayland.overlays.iter().filter(|o| o.owner == id) {
            if let (Some(surface), Some(output)) =
                (overlay.surface.wl_surface(), self.output_of(id))
            {
                let sent = output.clone();
                send_frames_surface_tree(&surface, &output, time, Some(Duration::ZERO), |_, _| {
                    Some(sent.clone())
                });
            }
        }
    }

    fn x11_fullscreen(&mut self, surface: &X11Surface, fullscreen: bool) {
        let Some(id) = self.x11_window_of(surface) else {
            return;
        };
        let Some(window) = self.windows.iter_mut().find(|window| window.id == id) else {
            return;
        };
        if window.fullscreen == fullscreen {
            let _ = surface.configure(None);
            return;
        }
        window.fullscreen = fullscreen;
        let _ = surface.set_fullscreen(fullscreen);
        self.events.push(ServerEvent::Fullscreen(id, fullscreen));
    }

    fn x11_grab(&mut self, surface: &X11Surface, grab: Grab) {
        if !self.buttons_held() {
            return;
        }
        if let Some(id) = self.x11_window_of(surface) {
            self.events.push(ServerEvent::Grabbed(id, grab));
        }
    }

    fn x11_maximized(&mut self, surface: &X11Surface, maximized: bool) {
        let Some(id) = self.x11_window_of(surface) else {
            return;
        };
        if let Some(window) = self.windows.iter_mut().find(|window| window.id == id) {
            window.maximized = maximized;
        }
        let _ = surface.set_maximized(maximized);
        let _ = surface.configure(None);
    }
}

impl XWaylandShellHandler for State {
    fn xwayland_shell_state(&mut self) -> &mut XWaylandShellState {
        &mut self.xwayland.shell
    }

    fn surface_associated(&mut self, _xwm: XwmId, _surface: WlSurface, window: X11Surface) {
        let owner = self.x11_window_of(&window).or_else(|| {
            self.xwayland
                .overlays
                .iter()
                .find(|overlay| overlay.surface == window)
                .map(|overlay| overlay.owner)
        });
        if let Some(id) = owner {
            self.redraw(id);
        }
    }
}

impl XwmHandler for State {
    fn xwm_state(&mut self, _xwm: XwmId) -> &mut X11Wm {
        self.xwayland
            .wm
            .as_mut()
            .expect("X11 events arrive only while the window manager runs")
    }

    fn new_window(&mut self, _xwm: XwmId, _window: X11Surface) {}

    fn new_override_redirect_window(&mut self, _xwm: XwmId, _window: X11Surface) {}

    fn map_window_request(&mut self, _xwm: XwmId, window: X11Surface) {
        if window.set_mapped(true).is_ok() {
            self.open_x11(window);
        }
    }

    fn mapped_override_redirect_window(&mut self, _xwm: XwmId, window: X11Surface) {
        self.xwayland
            .overlays
            .retain(|overlay| overlay.surface != window);
        let Some(owner) = self.overlay_owner(&window) else {
            return;
        };
        self.xwayland.overlays.push(Overlay {
            surface: window,
            owner,
        });
        self.redraw(owner);
    }

    fn unmapped_window(&mut self, _xwm: XwmId, window: X11Surface) {
        if window.is_override_redirect() {
            let owner = self
                .xwayland
                .overlays
                .iter()
                .find(|overlay| overlay.surface == window)
                .map(|overlay| overlay.owner);
            self.close_x11(&window);
            if let Some(owner) = owner {
                self.redraw(owner);
            }
            return;
        }
        self.close_x11(&window);
        let _ = window.set_mapped(false);
    }

    fn destroyed_window(&mut self, _xwm: XwmId, window: X11Surface) {
        self.close_x11(&window);
    }

    fn configure_request(
        &mut self,
        _xwm: XwmId,
        window: X11Surface,
        x: Option<i32>,
        y: Option<i32>,
        w: Option<u32>,
        h: Option<u32>,
        _reorder: Option<Reorder>,
    ) {
        if self.x11_window_of(&window).is_some() {
            let _ = window.configure(None);
            return;
        }
        let mut geometry = window.geometry();
        if let Some(x) = x {
            geometry.loc.x = x;
        }
        if let Some(y) = y {
            geometry.loc.y = y;
        }
        if let Some(w) = w {
            geometry.size.w = w as i32;
        }
        if let Some(h) = h {
            geometry.size.h = h as i32;
        }
        let _ = window.configure(geometry);
    }

    fn configure_notify(
        &mut self,
        _xwm: XwmId,
        window: X11Surface,
        _geometry: Rectangle<i32, Logical>,
        _above: Option<X11Window>,
    ) {
        if !window.is_override_redirect() {
            return;
        }
        let owner = self
            .xwayland
            .overlays
            .iter()
            .find(|overlay| overlay.surface == window)
            .map(|overlay| overlay.owner);
        if let Some(owner) = owner {
            self.redraw(owner);
        }
    }

    fn property_notify(&mut self, _xwm: XwmId, window: X11Surface, property: WmWindowProperty) {
        let Some(id) = self.x11_window_of(&window) else {
            return;
        };
        match property {
            WmWindowProperty::Title => {
                let title = window.title();
                if let Some(client) = self.windows.iter_mut().find(|client| client.id == id) {
                    client.title = title.clone();
                }
                self.events.push(ServerEvent::Titled(id, title));
            }
            WmWindowProperty::Class
            | WmWindowProperty::TransientFor
            | WmWindowProperty::NormalHints => {
                self.events.push(ServerEvent::Changed(id));
            }
            _ => {}
        }
    }

    fn fullscreen_request(&mut self, _xwm: XwmId, window: X11Surface) {
        self.x11_fullscreen(&window, true);
    }

    fn unfullscreen_request(&mut self, _xwm: XwmId, window: X11Surface) {
        self.x11_fullscreen(&window, false);
    }

    fn maximize_request(&mut self, _xwm: XwmId, window: X11Surface) {
        self.x11_maximized(&window, true);
    }

    fn unmaximize_request(&mut self, _xwm: XwmId, window: X11Surface) {
        self.x11_maximized(&window, false);
    }

    fn resize_request(&mut self, _xwm: XwmId, window: X11Surface, _button: u32, edge: ResizeEdge) {
        self.x11_grab(&window, Grab::Resize(x11_edge(edge)));
    }

    fn move_request(&mut self, _xwm: XwmId, window: X11Surface, _button: u32) {
        self.x11_grab(&window, Grab::Move);
    }

    fn minimize_request(&mut self, _xwm: XwmId, window: X11Surface) {
        if let Some(id) = self.x11_window_of(&window) {
            self.events.push(ServerEvent::Minimized(id));
        }
    }

    fn allow_selection_access(&mut self, _xwm: XwmId, _selection: SelectionTarget) -> bool {
        true
    }

    fn send_selection(
        &mut self,
        _xwm: XwmId,
        selection: SelectionTarget,
        mime_type: String,
        fd: OwnedFd,
    ) {
        self.x11_paste(selection, mime_type, fd);
    }

    fn new_selection(&mut self, _xwm: XwmId, selection: SelectionTarget, mime_types: Vec<String>) {
        self.x11_selected(selection, mime_types);
    }

    fn cleared_selection(&mut self, _xwm: XwmId, selection: SelectionTarget) {
        self.x11_cleared(selection);
    }

    fn disconnected(&mut self, _xwm: XwmId) {
        self.xwayland.stopped = true;
    }
}

fn x11_edge(edge: ResizeEdge) -> beui::WindowEdge {
    use beui::WindowEdge;
    match edge {
        ResizeEdge::Top => WindowEdge::Top,
        ResizeEdge::Bottom => WindowEdge::Bottom,
        ResizeEdge::Left => WindowEdge::Left,
        ResizeEdge::Right => WindowEdge::Right,
        ResizeEdge::TopLeft => WindowEdge::TopLeft,
        ResizeEdge::TopRight => WindowEdge::TopRight,
        ResizeEdge::BottomLeft => WindowEdge::BottomLeft,
        ResizeEdge::BottomRight => WindowEdge::BottomRight,
    }
}

delegate_xwayland_shell!(State);

#[cfg(test)]
mod tests;
