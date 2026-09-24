use std::time::{Duration, Instant};

use smithay::backend::renderer::utils::{RendererSurfaceStateUserData, on_commit_buffer_handler};
use std::os::fd::{AsFd, BorrowedFd, OwnedFd};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use smithay::backend::allocator::dmabuf::Dmabuf;
use smithay::backend::allocator::{Buffer as _, Format};
use smithay::delegate_compositor;
use smithay::delegate_dmabuf;
use smithay::wayland::compositor::{
    Blocker, BlockerState, BufferAssignment, SurfaceAttributes, add_blocker, add_pre_commit_hook,
};
use smithay::wayland::dmabuf::{
    DmabufFeedbackBuilder, DmabufGlobal, DmabufHandler, DmabufState, ImportNotifier, get_dmabuf,
};

use crate::server::{Waiter, readable};
use smithay::delegate_cursor_shape;
use smithay::delegate_data_device;
use smithay::delegate_output;
use smithay::delegate_seat;
use smithay::delegate_shm;
use smithay::delegate_viewporter;
use smithay::delegate_xdg_decoration;
use smithay::delegate_xdg_shell;
use smithay::desktop::{
    PopupKind, PopupManager, Window, WindowSurfaceType, find_popup_root_surface,
};
use smithay::input::keyboard::{FilterResult, Keycode, XkbConfig};
use smithay::input::pointer::{
    AxisFrame, ButtonEvent, CursorImageStatus, MotionEvent, PointerHandle,
};
use smithay::input::{Seat, SeatHandler, SeatState};
use smithay::output::{Mode, Output, PhysicalProperties, Scale, Subpixel};
use smithay::reexports::wayland_protocols::xdg::decoration::zv1::server::zxdg_toplevel_decoration_v1::Mode as DecorationMode;
use smithay::reexports::wayland_protocols::xdg::shell::server::xdg_toplevel;
use smithay::reexports::wayland_server::backend::{ClientData, ClientId, DisconnectReason};
use smithay::reexports::wayland_server::protocol::wl_buffer::WlBuffer;
use smithay::reexports::wayland_server::protocol::wl_seat::WlSeat;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::reexports::wayland_server::{Client, DisplayHandle, Resource};
use smithay::utils::{Logical, Point, Rectangle, SERIAL_COUNTER, Serial, Size, Transform};
use smithay::wayland::buffer::BufferHandler;
use smithay::wayland::compositor::{
    CompositorClientState, CompositorHandler, CompositorState, TraversalAction, get_parent,
    send_surface_state, with_states, with_surface_tree_downward,
};
use smithay::wayland::cursor_shape::CursorShapeManagerState;
use smithay::wayland::output::{OutputHandler, OutputManagerState};
use smithay::wayland::selection::SelectionHandler;
use smithay::wayland::selection::data_device::{
    ClientDndGrabHandler, DataDeviceHandler, DataDeviceState, ServerDndGrabHandler,
};
use smithay::wayland::shell::xdg::decoration::{XdgDecorationHandler, XdgDecorationState};
use smithay::wayland::shell::xdg::{
    PopupSurface, PositionerState, ToplevelSurface, XdgShellHandler, XdgShellState,
    XdgToplevelSurfaceData,
};
use smithay::wayland::shm::{ShmHandler, ShmState};
use smithay::wayland::viewporter::ViewporterState;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct WindowId(pub u64);

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ServerEvent {
    Opened(WindowId),
    Closed(WindowId),
    Titled(WindowId, String),
    Committed(WindowId),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Layer {
    pub surface: WlSurface,
    pub rect: Rectangle<f64, Logical>,
    pub source: Rectangle<f64, Logical>,
    pub size: Size<f64, Logical>,
}

struct ClientWindow {
    id: WindowId,
    window: Window,
    title: String,
}

#[derive(Default)]
pub struct ClientState {
    pub compositor: CompositorClientState,
}

impl ClientData for ClientState {
    fn initialized(&self, _client_id: ClientId) {}
    fn disconnected(&self, _client_id: ClientId, _reason: DisconnectReason) {}
}

pub struct State {
    handle: DisplayHandle,
    waiter: Waiter,
    blocked: Vec<Blocked>,
    dmabuf: DmabufState,
    dmabuf_formats: Vec<Format>,
    dmabuf_check: Option<Box<dyn FnMut(&Dmabuf) -> bool>>,
    start: Instant,
    compositor: CompositorState,
    xdg_shell: XdgShellState,
    shm: ShmState,
    seat_state: SeatState<State>,
    data_device: DataDeviceState,
    seat: Seat<State>,
    output: Output,
    popups: PopupManager,
    windows: Vec<ClientWindow>,
    next_window: u64,
    events: Vec<ServerEvent>,
    committed: Vec<WlSurface>,
    cursor: CursorImageStatus,
    scale: i32,
    size: Size<i32, Logical>,
    pointer_window: Option<WindowId>,
    keyboard_window: Option<WindowId>,
}

impl State {
    pub fn new(handle: &DisplayHandle, waiter: Waiter) -> Self {
        let compositor = CompositorState::new::<Self>(handle);
        let xdg_shell = XdgShellState::new::<Self>(handle);
        let shm = ShmState::new::<Self>(handle, Vec::new());
        let data_device = DataDeviceState::new::<Self>(handle);
        XdgDecorationState::new::<Self>(handle);
        CursorShapeManagerState::new::<Self>(handle);
        ViewporterState::new::<Self>(handle);
        OutputManagerState::new_with_xdg_output::<Self>(handle);
        let mut seat_state = SeatState::new();
        let mut seat = seat_state.new_wl_seat(handle, "seat0");
        seat.add_keyboard(XkbConfig::default(), 600, 25)
            .expect("the default keymap compiles");
        seat.add_pointer();
        let output = Output::new(
            "be-compositor".to_owned(),
            PhysicalProperties {
                size: (0, 0).into(),
                subpixel: Subpixel::Unknown,
                make: "BE3".to_owned(),
                model: "Nested".to_owned(),
            },
        );
        output.create_global::<Self>(handle);
        let state = Self {
            waiter,
            blocked: Vec::new(),
            dmabuf: DmabufState::new(),
            dmabuf_formats: Vec::new(),
            dmabuf_check: None,
            handle: handle.clone(),
            start: Instant::now(),
            compositor,
            xdg_shell,
            shm,
            seat_state,
            data_device,
            seat,
            output,
            popups: PopupManager::default(),
            windows: Vec::new(),
            next_window: 1,
            events: Vec::new(),
            committed: Vec::new(),
            cursor: CursorImageStatus::default_named(),
            scale: 1,
            size: Size::from((1280, 800)),
            pointer_window: None,
            keyboard_window: None,
        };
        state.set_output(state.size, state.scale);
        state
    }

    pub fn waiter(&self) -> &Waiter {
        &self.waiter
    }

    pub fn enable_dmabuf(
        &mut self,
        formats: Vec<Format>,
        render_node: Option<u64>,
        check: Option<Box<dyn FnMut(&Dmabuf) -> bool>>,
    ) {
        if formats.is_empty() {
            return;
        }
        self.dmabuf_formats = formats.clone();
        self.dmabuf_check = check;
        let handle = self.handle.clone();
        let feedback = render_node.and_then(|node| {
            DmabufFeedbackBuilder::new(node, formats.clone())
                .build()
                .ok()
        });
        match feedback {
            Some(feedback) => {
                self.dmabuf
                    .create_global_with_default_feedback::<Self>(&handle, &feedback);
            }
            None => {
                self.dmabuf.create_global::<Self>(&handle, formats);
            }
        }
    }

    fn pre_commit(&mut self, surface: &WlSurface) {
        let dmabuf = with_states(surface, |states| {
            let mut cached = states.cached_state.get::<SurfaceAttributes>();
            match cached.pending().buffer.as_ref() {
                Some(BufferAssignment::NewBuffer(buffer)) => get_dmabuf(buffer).cloned().ok(),
                _ => None,
            }
        });
        if let Some(dmabuf) = dmabuf {
            for fd in dmabuf.handles() {
                self.block_on(surface, fd);
            }
        }
    }

    fn block_on(&mut self, surface: &WlSurface, fd: BorrowedFd<'_>) {
        if readable(fd) {
            return;
        }
        let (Some(client), Ok(watched), Ok(kept)) = (
            surface.client(),
            fd.try_clone_to_owned(),
            fd.try_clone_to_owned(),
        ) else {
            return;
        };
        let ready = Arc::new(AtomicBool::new(false));
        add_blocker(surface, FlagBlocker(ready.clone()));
        self.waiter.wait(watched, ready.clone());
        self.blocked.push(Blocked {
            ready,
            fd: kept,
            client,
        });
    }

    pub fn blocked(&self) -> usize {
        self.blocked.len()
    }

    pub fn release_blockers(&mut self) {
        let (released, waiting): (Vec<Blocked>, Vec<Blocked>) = std::mem::take(&mut self.blocked)
            .into_iter()
            .partition(|blocked| {
                blocked.ready.load(Ordering::SeqCst) || readable(blocked.fd.as_fd())
            });
        self.blocked = waiting;
        let handle = self.handle.clone();
        for blocked in released {
            blocked.ready.store(true, Ordering::SeqCst);
            if let Some(data) = blocked.client.get_data::<ClientState>() {
                data.compositor.blocker_cleared(self, &handle);
            }
        }
    }

    pub fn cleanup(&mut self) {
        self.popups.cleanup();
    }

    pub fn take_events(&mut self) -> Vec<ServerEvent> {
        std::mem::take(&mut self.events)
    }

    pub fn take_committed(&mut self) -> Vec<WlSurface> {
        std::mem::take(&mut self.committed)
    }

    pub fn cursor(&self) -> &CursorImageStatus {
        &self.cursor
    }

    pub fn windows(&self) -> Vec<WindowId> {
        self.windows.iter().map(|window| window.id).collect()
    }

    pub fn title(&self, id: WindowId) -> Option<String> {
        self.find(id).map(|window| window.title.clone())
    }

    pub fn set_output(&self, size: Size<i32, Logical>, scale: i32) {
        let mode = Mode {
            size: (size.w.max(1) * scale, size.h.max(1) * scale).into(),
            refresh: 60_000,
        };
        self.output.change_current_state(
            Some(mode),
            Some(Transform::Normal),
            Some(Scale::Integer(scale)),
            Some((0, 0).into()),
        );
        self.output.set_preferred(mode);
    }

    pub fn set_scale(&mut self, scale: i32, size: Size<i32, Logical>) {
        if (scale, size) == (self.scale, self.size) {
            return;
        }
        self.set_output(size, scale);
        self.size = size;
        if scale == self.scale {
            return;
        }
        self.scale = scale;
        for window in &self.windows {
            window.window.with_surfaces(|surface, data| {
                send_surface_state(surface, data, scale, Transform::Normal);
            });
        }
    }

    fn find(&self, id: WindowId) -> Option<&ClientWindow> {
        self.windows.iter().find(|window| window.id == id)
    }

    fn window_of_root(&self, root: &WlSurface) -> Option<&ClientWindow> {
        self.windows.iter().find(|window| {
            window
                .window
                .toplevel()
                .is_some_and(|toplevel| toplevel.wl_surface() == root)
        })
    }

    fn toplevel(&self, id: WindowId) -> Option<ToplevelSurface> {
        self.find(id)?.window.toplevel().cloned()
    }

    pub fn surface(&self, id: WindowId) -> Option<WlSurface> {
        Some(self.toplevel(id)?.wl_surface().clone())
    }

    pub fn configure(&mut self, id: WindowId, size: Size<i32, Logical>, activated: bool) {
        let Some(toplevel) = self.toplevel(id) else {
            return;
        };
        toplevel.with_pending_state(|state| {
            state.size = Some(size);
            state.bounds = Some(size);
            for tiled in [
                xdg_toplevel::State::TiledLeft,
                xdg_toplevel::State::TiledRight,
                xdg_toplevel::State::TiledTop,
                xdg_toplevel::State::TiledBottom,
            ] {
                state.states.set(tiled);
            }
            if activated {
                state.states.set(xdg_toplevel::State::Activated);
            } else {
                state.states.unset(xdg_toplevel::State::Activated);
            }
        });
        if toplevel.is_initial_configure_sent() {
            toplevel.send_pending_configure();
        }
    }

    pub fn mapped_size(&self, id: WindowId) -> Option<beui::Vec2> {
        let window = self.find(id)?;
        if self.layers(id).is_empty() {
            return None;
        }
        let size = window.window.geometry().size;
        (size.w > 0 && size.h > 0).then(|| beui::vec2(size.w as f32, size.h as f32))
    }

    pub fn close(&self, id: WindowId) {
        if let Some(toplevel) = self.toplevel(id) {
            toplevel.send_close();
        }
    }

    pub fn layers(&self, id: WindowId) -> Vec<Layer> {
        let Some(window) = self.find(id) else {
            return Vec::new();
        };
        let Some(toplevel) = window.window.toplevel() else {
            return Vec::new();
        };
        let mut layers = Vec::new();
        let origin = Point::from((0, 0)) - window.window.geometry().loc;
        collect_layers(toplevel.wl_surface(), origin, &mut layers);
        for (popup, offset) in PopupManager::popups_for_surface(toplevel.wl_surface()) {
            let location = offset - popup.geometry().loc;
            collect_layers(popup.wl_surface(), location, &mut layers);
        }
        layers
    }

    pub fn send_frames(&self, id: WindowId) {
        let Some(window) = self.find(id) else {
            return;
        };
        let output = self.output.clone();
        window.window.send_frame(
            &self.output,
            self.start.elapsed(),
            Some(Duration::ZERO),
            |_, _| Some(output.clone()),
        );
    }

    fn time(&self) -> u32 {
        self.start.elapsed().as_millis() as u32
    }

    fn pointer(&self) -> PointerHandle<Self> {
        self.seat.get_pointer().expect("the seat has a pointer")
    }

    pub fn pointer_window(&self) -> Option<WindowId> {
        self.pointer_window
    }

    pub fn pointer_motion(&mut self, target: Option<(WindowId, Point<f64, Logical>)>) {
        let focus = target.and_then(|(id, position)| {
            let window = self.find(id)?;
            let location = position + window.window.geometry().loc.to_f64();
            let under = window
                .window
                .surface_under(location, WindowSurfaceType::ALL)
                .map(|(surface, origin)| (surface, origin.to_f64()));
            Some((id, location, under))
        });
        let pointer = self.pointer();
        let serial = SERIAL_COUNTER.next_serial();
        let time = self.time();
        match focus {
            Some((id, location, under)) => {
                self.pointer_window = Some(id);
                pointer.motion(
                    self,
                    under,
                    &MotionEvent {
                        location,
                        serial,
                        time,
                    },
                );
            }
            None => {
                self.pointer_window = None;
                let location = pointer.current_location();
                pointer.motion(
                    self,
                    None,
                    &MotionEvent {
                        location,
                        serial,
                        time,
                    },
                );
            }
        }
        pointer.frame(self);
    }

    pub fn pointer_button(&mut self, button: u32, pressed: bool) {
        let pointer = self.pointer();
        if pressed {
            self.dismiss_popups_outside(&pointer);
        }
        let serial = SERIAL_COUNTER.next_serial();
        let time = self.time();
        pointer.button(
            self,
            &ButtonEvent {
                serial,
                time,
                button,
                state: if pressed {
                    smithay::backend::input::ButtonState::Pressed
                } else {
                    smithay::backend::input::ButtonState::Released
                },
            },
        );
        pointer.frame(self);
    }

    fn dismiss_popups_outside(&mut self, pointer: &PointerHandle<Self>) {
        let over = pointer.current_focus();
        for window in &self.windows {
            let Some(toplevel) = window.window.toplevel() else {
                continue;
            };
            let popups: Vec<PopupKind> = PopupManager::popups_for_surface(toplevel.wl_surface())
                .map(|(popup, _)| popup)
                .collect();
            let inside = popups
                .iter()
                .any(|popup| over.as_ref().is_some_and(|over| over == popup.wl_surface()));
            if inside {
                continue;
            }
            for popup in popups.iter().rev() {
                let _ = PopupManager::dismiss_popup(toplevel.wl_surface(), popup);
            }
        }
    }

    pub fn pointer_axis(&mut self, delta: (f64, f64)) {
        let pointer = self.pointer();
        let mut frame =
            AxisFrame::new(self.time()).source(smithay::backend::input::AxisSource::Continuous);
        if delta.0 != 0.0 {
            frame = frame.value(smithay::backend::input::Axis::Horizontal, delta.0);
        }
        if delta.1 != 0.0 {
            frame = frame.value(smithay::backend::input::Axis::Vertical, delta.1);
        }
        pointer.axis(self, frame);
        pointer.frame(self);
    }

    pub fn keyboard_window(&self) -> Option<WindowId> {
        self.keyboard_window
    }

    pub fn focus_keyboard(&mut self, id: Option<WindowId>) {
        if id == self.keyboard_window {
            return;
        }
        self.keyboard_window = id;
        let surface = id
            .and_then(|id| self.toplevel(id))
            .map(|toplevel| toplevel.wl_surface().clone());
        let keyboard = self.seat.get_keyboard().expect("the seat has a keyboard");
        keyboard.set_focus(self, surface, SERIAL_COUNTER.next_serial());
    }

    pub fn key(&mut self, code: u32, pressed: bool) {
        let keyboard = self.seat.get_keyboard().expect("the seat has a keyboard");
        let time = self.time();
        keyboard.input::<(), _>(
            self,
            Keycode::new(code + 8),
            if pressed {
                smithay::backend::input::KeyState::Pressed
            } else {
                smithay::backend::input::KeyState::Released
            },
            SERIAL_COUNTER.next_serial(),
            time,
            |_, _, _| FilterResult::Forward,
        );
    }

    fn opened(&mut self, toplevel: ToplevelSurface) {
        let id = WindowId(self.next_window);
        self.next_window += 1;
        self.windows.push(ClientWindow {
            id,
            window: Window::new_wayland_window(toplevel),
            title: String::new(),
        });
        self.events.push(ServerEvent::Opened(id));
    }

    fn committed_window(&mut self, root: &WlSurface) {
        let Some(window) = self.window_of_root(root) else {
            return;
        };
        let id = window.id;
        window.window.on_commit();
        let output = self.output.clone();
        let scale = self.scale;
        window.window.with_surfaces(|surface, data| {
            output.enter(surface);
            send_surface_state(surface, data, scale, Transform::Normal);
        });
        if let Some(toplevel) = window.window.toplevel()
            && !toplevel.is_initial_configure_sent()
        {
            toplevel.send_configure();
        }
        if !self.events.contains(&ServerEvent::Committed(id)) {
            self.events.push(ServerEvent::Committed(id));
        }
    }
}

fn collect_layers(surface: &WlSurface, location: Point<i32, Logical>, layers: &mut Vec<Layer>) {
    with_surface_tree_downward(
        surface,
        location,
        |_, states, location| {
            let data = states.data_map.get::<RendererSurfaceStateUserData>();
            match data.and_then(|data| data.lock().unwrap().view()) {
                Some(view) => TraversalAction::DoChildren(*location + view.offset),
                None => TraversalAction::SkipChildren,
            }
        },
        |surface, states, location| {
            let Some(data) = states.data_map.get::<RendererSurfaceStateUserData>() else {
                return;
            };
            let data = data.lock().unwrap();
            let (Some(view), Some(size)) = (data.view(), data.surface_size()) else {
                return;
            };
            let origin = *location + view.offset;
            layers.push(Layer {
                surface: surface.clone(),
                rect: Rectangle::new(origin.to_f64(), view.dst.to_f64()),
                source: view.src,
                size: size.to_f64(),
            });
        },
        |_, _, _| true,
    );
}

struct Blocked {
    ready: Arc<AtomicBool>,
    fd: OwnedFd,
    client: Client,
}

struct FlagBlocker(Arc<AtomicBool>);

impl Blocker for FlagBlocker {
    fn state(&self) -> BlockerState {
        match self.0.load(Ordering::SeqCst) {
            true => BlockerState::Released,
            false => BlockerState::Pending,
        }
    }
}

impl DmabufHandler for State {
    fn dmabuf_state(&mut self) -> &mut DmabufState {
        &mut self.dmabuf
    }

    fn dmabuf_imported(
        &mut self,
        _global: &DmabufGlobal,
        dmabuf: Dmabuf,
        notifier: ImportNotifier,
    ) {
        let known = self.dmabuf_formats.contains(&dmabuf.format());
        let imported = known
            && self
                .dmabuf_check
                .as_mut()
                .is_none_or(|check| check(&dmabuf));
        if imported {
            let _ = notifier.successful::<Self>();
        } else {
            notifier.failed();
        }
    }
}

impl CompositorHandler for State {
    fn new_surface(&mut self, surface: &WlSurface) {
        add_pre_commit_hook::<Self, _>(surface, |state, _handle, surface| {
            state.pre_commit(surface);
        });
    }

    fn compositor_state(&mut self) -> &mut CompositorState {
        &mut self.compositor
    }

    fn client_compositor_state<'a>(&self, client: &'a Client) -> &'a CompositorClientState {
        &client
            .get_data::<ClientState>()
            .expect("every client is inserted with a ClientState")
            .compositor
    }

    fn commit(&mut self, surface: &WlSurface) {
        on_commit_buffer_handler::<Self>(surface);
        self.popups.commit(surface);
        if let Some(PopupKind::Xdg(popup)) = self.popups.find_popup(surface)
            && !popup.is_initial_configure_sent()
        {
            let _ = popup.send_configure();
        }
        self.committed.push(surface.clone());
        let mut root = surface.clone();
        while let Some(parent) = get_parent(&root) {
            root = parent;
        }
        if let Some(popup) = self.popups.find_popup(&root)
            && let Ok(toplevel) = find_popup_root_surface(&popup)
        {
            root = toplevel;
        }
        self.committed_window(&root);
    }
}

impl BufferHandler for State {
    fn buffer_destroyed(&mut self, _buffer: &WlBuffer) {}
}

impl ShmHandler for State {
    fn shm_state(&self) -> &ShmState {
        &self.shm
    }
}

impl XdgShellHandler for State {
    fn xdg_shell_state(&mut self) -> &mut XdgShellState {
        &mut self.xdg_shell
    }

    fn new_toplevel(&mut self, surface: ToplevelSurface) {
        surface.with_pending_state(|state| {
            state.decoration_mode = Some(DecorationMode::ServerSide);
        });
        self.opened(surface);
    }

    fn new_popup(&mut self, surface: PopupSurface, positioner: PositionerState) {
        let geometry = self.constrained(&surface, positioner);
        surface.with_pending_state(|state| state.geometry = geometry);
        let _ = self.popups.track_popup(PopupKind::Xdg(surface));
    }

    fn reposition_request(
        &mut self,
        surface: PopupSurface,
        positioner: PositionerState,
        token: u32,
    ) {
        let geometry = self.constrained(&surface, positioner);
        surface.with_pending_state(|state| {
            state.geometry = geometry;
            state.positioner = positioner;
        });
        surface.send_repositioned(token);
    }

    fn grab(&mut self, _surface: PopupSurface, _seat: WlSeat, _serial: Serial) {}

    fn toplevel_destroyed(&mut self, surface: ToplevelSurface) {
        let Some(index) = self
            .windows
            .iter()
            .position(|window| window.window.toplevel() == Some(&surface))
        else {
            return;
        };
        let id = self.windows.remove(index).id;
        if self.keyboard_window == Some(id) {
            self.keyboard_window = None;
        }
        if self.pointer_window == Some(id) {
            self.pointer_window = None;
        }
        self.events.push(ServerEvent::Closed(id));
    }

    fn title_changed(&mut self, surface: ToplevelSurface) {
        let title = with_states(surface.wl_surface(), |states| {
            states
                .data_map
                .get::<XdgToplevelSurfaceData>()
                .and_then(|data| data.lock().unwrap().title.clone())
        })
        .unwrap_or_default();
        let Some(window) = self
            .windows
            .iter_mut()
            .find(|window| window.window.toplevel() == Some(&surface))
        else {
            return;
        };
        window.title = title.clone();
        let id = window.id;
        self.events.push(ServerEvent::Titled(id, title));
    }
}

impl State {
    fn constrained(
        &self,
        popup: &PopupSurface,
        positioner: PositionerState,
    ) -> Rectangle<i32, Logical> {
        let Ok(root) = find_popup_root_surface(&PopupKind::Xdg(popup.clone())) else {
            return positioner.get_geometry();
        };
        let Some(window) = self.window_of_root(&root) else {
            return positioner.get_geometry();
        };
        let geometry = window.window.geometry();
        let parent = popup
            .get_parent_surface()
            .map(|parent| parent_offset(&parent, &root))
            .unwrap_or_default();
        let target = Rectangle::new(Point::default() - parent, geometry.size);
        positioner.get_unconstrained_geometry(target)
    }
}

fn parent_offset(parent: &WlSurface, root: &WlSurface) -> Point<i32, Logical> {
    if parent == root {
        return Point::default();
    }
    let popup = with_states(parent, |states| {
        states
            .data_map
            .get::<smithay::wayland::shell::xdg::XdgPopupSurfaceData>()
            .map(|data| {
                let data = data.lock().unwrap();
                (data.parent.clone(), data.current.geometry.loc)
            })
    });
    match popup {
        Some((Some(grand), location)) => parent_offset(&grand, root) + location,
        _ => Point::default(),
    }
}

impl XdgDecorationHandler for State {
    fn new_decoration(&mut self, toplevel: ToplevelSurface) {
        toplevel.with_pending_state(|state| {
            state.decoration_mode = Some(DecorationMode::ServerSide);
        });
        if toplevel.is_initial_configure_sent() {
            toplevel.send_pending_configure();
        }
    }

    fn request_mode(&mut self, toplevel: ToplevelSurface, _mode: DecorationMode) {
        self.new_decoration(toplevel);
    }

    fn unset_mode(&mut self, toplevel: ToplevelSurface) {
        self.new_decoration(toplevel);
    }
}

impl SeatHandler for State {
    type KeyboardFocus = WlSurface;
    type PointerFocus = WlSurface;
    type TouchFocus = WlSurface;

    fn seat_state(&mut self) -> &mut SeatState<Self> {
        &mut self.seat_state
    }

    fn cursor_image(&mut self, _seat: &Seat<Self>, image: CursorImageStatus) {
        self.cursor = image;
    }

    fn focus_changed(&mut self, seat: &Seat<Self>, focused: Option<&WlSurface>) {
        let client = focused.and_then(|surface| self.handle_of(surface));
        smithay::wayland::selection::data_device::set_data_device_focus(
            &self.display_handle(),
            seat,
            client,
        );
    }
}

impl State {
    fn handle_of(&self, surface: &WlSurface) -> Option<Client> {
        surface.client()
    }

    fn display_handle(&self) -> DisplayHandle {
        self.handle.clone()
    }
}

impl SelectionHandler for State {
    type SelectionUserData = ();
}

impl DataDeviceHandler for State {
    fn data_device_state(&self) -> &DataDeviceState {
        &self.data_device
    }
}

impl ClientDndGrabHandler for State {}

impl ServerDndGrabHandler for State {}

impl OutputHandler for State {}

impl smithay::wayland::tablet_manager::TabletSeatHandler for State {}

delegate_compositor!(State);
delegate_xdg_shell!(State);
delegate_shm!(State);
delegate_seat!(State);
delegate_data_device!(State);
delegate_output!(State);
delegate_xdg_decoration!(State);
delegate_cursor_shape!(State);
delegate_viewporter!(State);
delegate_dmabuf!(State);
