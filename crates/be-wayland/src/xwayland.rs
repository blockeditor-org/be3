use std::os::unix::net::UnixStream;
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
use smithay::wayland::xwayland_shell::{XWaylandShellHandler, XWaylandShellState};
use smithay::xwayland::xwm::{Reorder, ResizeEdge, WmWindowProperty, X11Window, XwmId};
use smithay::xwayland::{X11Surface, X11Wm, XWayland, XWaylandEvent, XwmHandler};

use crate::state::{ClientWindow, Layer, ServerEvent, State, WindowId, collect_layers};

const MAX_RESTARTS: u32 = 3;
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
    overlays: Vec<Overlay>,
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
            overlays: Vec::new(),
        }
    }

    pub(crate) fn display(&self) -> Option<u32> {
        self.display
    }
}

impl State {
    pub(crate) fn start_xwayland(&mut self) -> std::io::Result<u32> {
        let wanted = self.xwayland.display;
        let (xwayland, client) = XWayland::spawn(
            &self.handle,
            wanted,
            std::iter::empty::<(String, String)>(),
            true,
            Stdio::null(),
            Stdio::null(),
            |_| (),
        )?;
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
            .map_err(|error| std::io::Error::other(error.error))?;
        self.xwayland.source = Some(source);
        self.xwayland.display = Some(display);
        Ok(display)
    }

    fn xwayland_ready(&mut self, connection: UnixStream, client: Client) {
        let handle = self.xwayland.loop_handle.clone();
        match X11Wm::start_wm(handle, connection, client) {
            Ok(wm) => self.xwayland.wm = Some(wm),
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
            return;
        }
        self.xwayland.restarts += 1;
        if let Err(error) = self.start_xwayland() {
            eprintln!("be-wayland: Xwayland did not start again: {error}");
            self.xwayland.display = None;
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
            if let (Some(surface), Some(output)) = (overlay.surface.wl_surface(), self.output_of(id)) {
                let sent = output.clone();
                send_frames_surface_tree(
                    &surface,
                    &output,
                    time,
                    Some(Duration::ZERO),
                    |_, _| Some(sent.clone()),
                );
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
            WmWindowProperty::Class | WmWindowProperty::TransientFor => {
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

    fn resize_request(
        &mut self,
        _xwm: XwmId,
        _window: X11Surface,
        _button: u32,
        _edge: ResizeEdge,
    ) {
    }

    fn move_request(&mut self, _xwm: XwmId, _window: X11Surface, _button: u32) {}

    fn disconnected(&mut self, _xwm: XwmId) {
        self.xwayland.stopped = true;
    }
}

delegate_xwayland_shell!(State);
