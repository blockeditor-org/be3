use std::io::Write as _;
use std::os::unix::net::UnixStream;

use wayland_client::protocol::{
    wl_buffer, wl_callback, wl_compositor, wl_keyboard, wl_output, wl_pointer, wl_registry,
    wl_seat, wl_shm, wl_shm_pool, wl_surface,
};
use wayland_client::{Connection, Dispatch, EventQueue, QueueHandle, WEnum};
use wayland_protocols::xdg::shell::client::{
    xdg_popup, xdg_positioner, xdg_surface, xdg_toplevel, xdg_wm_base,
};

use crate::server::Server;

use wayland_protocols::xdg::decoration::zv1::client::{
    zxdg_decoration_manager_v1, zxdg_toplevel_decoration_v1,
};
use wayland_protocols_misc::server_decoration::client::{
    org_kde_kwin_server_decoration, org_kde_kwin_server_decoration_manager,
};

use wayland_protocols::wp::linux_dmabuf::zv1::client::{
    zwp_linux_buffer_params_v1, zwp_linux_dmabuf_v1,
};

use wayland_protocols::ext::idle_notify::v1::client::{
    ext_idle_notification_v1, ext_idle_notifier_v1,
};
use wayland_protocols::wp::cursor_shape::v1::client::{
    wp_cursor_shape_device_v1, wp_cursor_shape_manager_v1,
};
use wayland_protocols::wp::idle_inhibit::zv1::client::{
    zwp_idle_inhibit_manager_v1, zwp_idle_inhibitor_v1,
};

mod protocols;
pub(crate) use protocols::PenSeen;
use wayland_protocols::wp::presentation_time::client::{wp_presentation, wp_presentation_feedback};
use wayland_protocols::xdg::activation::v1::client::{xdg_activation_token_v1, xdg_activation_v1};

const ARGB8888: u32 = u32::from_le_bytes(*b"AR24");
use crate::state::{ServerEvent, WindowId};

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Seen {
    Button(wl_surface::WlSurface, bool),
    Key(wl_surface::WlSurface, u32),
    PopupDone(xdg_popup::XdgPopup),
}

#[derive(Default)]
pub(crate) struct Received {
    pub(crate) globals: Vec<(u32, String, u32)>,
    pub(crate) entered: Vec<u32>,
    pub(crate) removed: Vec<u32>,
    pub(crate) output_names: Vec<(u32, String)>,
    pub(crate) outputs: Vec<(u32, wl_output::WlOutput)>,
    pub(crate) configured: Option<u32>,
    pub(crate) size: Option<(i32, i32)>,
    pub(crate) activated: bool,
    pub(crate) fullscreen: bool,
    pub(crate) maximized: bool,
    pub(crate) keyboard_entered: bool,
    pub(crate) keyboard_surface: Option<wl_surface::WlSurface>,
    pub(crate) serial: Option<u32>,
    pub(crate) seen: Vec<Seen>,
    pub(crate) keys: Vec<(u32, bool)>,
    pub(crate) keymaps: usize,
    pub(crate) repeat: Option<(i32, i32)>,
    pub(crate) pointer_entered: Option<(f64, f64)>,
    pub(crate) pointer_surface: Option<wl_surface::WlSurface>,
    pub(crate) pointer_serial: Option<u32>,
    pub(crate) buttons: Vec<(u32, bool)>,
    pub(crate) scrolled: f64,
    pub(crate) frames: usize,
    pub(crate) released: Vec<wl_buffer::WlBuffer>,
    pub(crate) xdg_decoration: Option<zxdg_toplevel_decoration_v1::Mode>,
    pub(crate) kde_default_decoration: Option<org_kde_kwin_server_decoration_manager::Mode>,
    pub(crate) kde_decoration: Option<org_kde_kwin_server_decoration::Mode>,
    pub(crate) ignores_pings: bool,
    pub(crate) pings: Vec<u32>,
    pub(crate) idle: Vec<bool>,
    pub(crate) protocols: protocols::Protocols,
    pub(crate) clock: Option<u32>,
    pub(crate) presented: Vec<Feedback>,
    pub(crate) discarded: usize,
    pub(crate) token: Option<String>,
    pub(crate) popup_geometry: Option<(i32, i32, i32, i32)>,
    pub(crate) repositioned: Option<u32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Feedback {
    pub(crate) time: std::time::Duration,
    pub(crate) refresh: u32,
    pub(crate) sequence: u64,
    pub(crate) flags: u32,
}

pub(crate) struct TestClient {
    pub(crate) connection: Connection,
    pub(crate) queue: EventQueue<Received>,
    pub(crate) handle: QueueHandle<Received>,
    pub(crate) received: Received,
    pub(crate) registry: wl_registry::WlRegistry,
    pub(crate) compositor: Option<wl_compositor::WlCompositor>,
    pub(crate) shm: Option<wl_shm::WlShm>,
    pub(crate) wm_base: Option<xdg_wm_base::XdgWmBase>,
    pub(crate) seat: Option<wl_seat::WlSeat>,
}

pub(crate) struct TestWindow {
    pub(crate) surface: wl_surface::WlSurface,
    pub(crate) xdg_surface: xdg_surface::XdgSurface,
    pub(crate) toplevel: Option<xdg_toplevel::XdgToplevel>,
    pub(crate) popup: Option<xdg_popup::XdgPopup>,
}

impl TestWindow {
    pub(crate) fn destroy(&self) {
        if let Some(popup) = &self.popup {
            popup.destroy();
        }
        if let Some(toplevel) = &self.toplevel {
            toplevel.destroy();
        }
        self.xdg_surface.destroy();
        self.surface.destroy();
    }
}

pub(crate) fn server() -> Server {
    Server::headless().expect("a headless display starts")
}

impl TestClient {
    pub(crate) fn connect(server: &mut Server) -> Self {
        let (client, host) = UnixStream::pair().expect("a socket pair opens");
        server.connect(host).expect("the server takes the client");
        let connection = Connection::from_socket(client).expect("the client connects");
        let queue = connection.new_event_queue();
        let handle = queue.handle();
        let registry = connection.display().get_registry(&handle, ());
        let mut client = Self {
            connection,
            queue,
            handle,
            received: Received::default(),
            registry,
            compositor: None,
            shm: None,
            wm_base: None,
            seat: None,
        };
        client.exchange(server);
        client.compositor = Some(client.bind("wl_compositor", 5));
        client.shm = Some(client.bind("wl_shm", 1));
        client.wm_base = Some(client.bind("xdg_wm_base", 5));
        client.seat = Some(client.bind("wl_seat", 7));
        client.exchange(server);
        client
    }

    pub(crate) fn bind<I>(&self, interface: &str, version: u32) -> I
    where
        I: wayland_client::Proxy + 'static,
        Received: Dispatch<I, ()>,
    {
        let (name, _, available) = self
            .received
            .globals
            .iter()
            .find(|(_, name, _)| name == interface)
            .unwrap_or_else(|| panic!("the server advertises {interface}"));
        self.registry
            .bind::<I, _, _>(*name, version.min(*available), &self.handle, ())
    }

    pub(crate) fn outputs(&self) -> Vec<u32> {
        self.received
            .globals
            .iter()
            .filter(|(_, interface, _)| interface == "wl_output")
            .map(|(name, _, _)| *name)
            .collect()
    }

    pub(crate) fn bind_outputs(&mut self) {
        for (name, interface, version) in self.received.globals.clone() {
            let bound = self.received.output_names.iter().any(|(held, _)| *held == name);
            if interface == "wl_output" && !bound {
                let output = self.registry.bind::<wl_output::WlOutput, u32, Received>(
                    name,
                    version.min(4),
                    &self.handle,
                    name,
                );
                self.received.outputs.push((name, output));
            }
        }
    }

    pub(crate) fn output_named(&self, wanted: &str) -> Option<wl_output::WlOutput> {
        let received = &self.received;
        let (global, _) = received
            .output_names
            .iter()
            .find(|(_, name)| name == wanted)?;
        received
            .outputs
            .iter()
            .find(|(held, _)| held == global)
            .map(|(_, output)| output.clone())
    }

    pub(crate) fn exchange(&mut self, server: &mut Server) {
        for _ in 0..2 {
            self.connection.flush().expect("the client flushes");
            server.dispatch();
            if let Some(guard) = self.queue.prepare_read() {
                let _ = guard.read();
            }
            self.queue
                .dispatch_pending(&mut self.received)
                .expect("the client dispatches");
        }
    }

    pub(crate) fn open(&mut self, server: &mut Server) -> (TestWindow, WindowId) {
        let surface = self
            .compositor
            .as_ref()
            .unwrap()
            .create_surface(&self.handle, ());
        let xdg_surface =
            self.wm_base
                .as_ref()
                .unwrap()
                .get_xdg_surface(&surface, &self.handle, ());
        let toplevel = xdg_surface.get_toplevel(&self.handle, ());
        surface.commit();
        self.exchange(server);
        let id = server
            .state
            .take_events()
            .into_iter()
            .find_map(|event| match event {
                ServerEvent::Opened(id) => Some(id),
                _ => None,
            })
            .expect("the toplevel opens a window");
        let serial = self
            .received
            .configured
            .take()
            .expect("the first commit is configured");
        xdg_surface.ack_configure(serial);
        (
            TestWindow {
                surface,
                xdg_surface,
                toplevel: Some(toplevel),
                popup: None,
            },
            id,
        )
    }

    pub(crate) fn toplevel(&mut self) -> TestWindow {
        let surface = self
            .compositor
            .as_ref()
            .unwrap()
            .create_surface(&self.handle, ());
        let xdg_surface =
            self.wm_base
                .as_ref()
                .unwrap()
                .get_xdg_surface(&surface, &self.handle, ());
        let toplevel = xdg_surface.get_toplevel(&self.handle, ());
        surface.commit();
        TestWindow {
            surface,
            xdg_surface,
            toplevel: Some(toplevel),
            popup: None,
        }
    }

    pub(crate) fn inhibit(
        &self,
        surface: &wl_surface::WlSurface,
    ) -> zwp_idle_inhibitor_v1::ZwpIdleInhibitorV1 {
        let manager: zwp_idle_inhibit_manager_v1::ZwpIdleInhibitManagerV1 =
            self.bind("zwp_idle_inhibit_manager_v1", 1);
        manager.create_inhibitor(surface, &self.handle, ())
    }

    pub(crate) fn presentation_feedback(
        &self,
        surface: &wl_surface::WlSurface,
    ) -> wp_presentation_feedback::WpPresentationFeedback {
        let presentation: wp_presentation::WpPresentation = self.bind("wp_presentation", 2);
        presentation.feedback(surface, &self.handle, ())
    }

    pub(crate) fn idle_notification(
        &self,
        timeout_ms: u32,
        input_only: bool,
    ) -> ext_idle_notification_v1::ExtIdleNotificationV1 {
        let notifier: ext_idle_notifier_v1::ExtIdleNotifierV1 =
            self.bind("ext_idle_notifier_v1", 2);
        let seat = self.seat.as_ref().unwrap();
        match input_only {
            true => notifier.get_input_idle_notification(timeout_ms, seat, &self.handle, ()),
            false => notifier.get_idle_notification(timeout_ms, seat, &self.handle, ()),
        }
    }

    pub(crate) fn request_token(
        &self,
        serial: Option<u32>,
        surface: Option<&wl_surface::WlSurface>,
    ) {
        let activation: xdg_activation_v1::XdgActivationV1 = self.bind("xdg_activation_v1", 1);
        let token = activation.get_activation_token(&self.handle, ());
        if let Some(serial) = serial {
            token.set_serial(serial, self.seat.as_ref().unwrap());
        }
        if let Some(surface) = surface {
            token.set_surface(surface);
        }
        token.commit();
    }

    pub(crate) fn activate(&self, token: &str, surface: &wl_surface::WlSurface) {
        let activation: xdg_activation_v1::XdgActivationV1 = self.bind("xdg_activation_v1", 1);
        activation.activate(token.to_owned(), surface);
    }

    pub(crate) fn flush(&mut self) {
        if self.disconnected() {
            return;
        }
        self.connection.flush().expect("the client flushes");
    }

    pub(crate) fn disconnected(&self) -> bool {
        self.connection.protocol_error().is_some()
    }

    pub(crate) fn pong(&mut self) {
        let base = self.wm_base.as_ref().unwrap();
        for serial in self.received.pings.drain(..) {
            base.pong(serial);
        }
    }

    pub(crate) fn receive(&mut self) {
        if self.disconnected() {
            return;
        }
        if let Some(guard) = self.queue.prepare_read() {
            let _ = guard.read();
        }
        if self.disconnected() {
            return;
        }
        self.queue
            .dispatch_pending(&mut self.received)
            .expect("the client dispatches");
    }

    pub(crate) fn attach_dmabuf_unsent(
        &mut self,
        window: &TestWindow,
        fd: std::os::fd::BorrowedFd<'_>,
        width: i32,
        height: i32,
    ) -> wl_buffer::WlBuffer {
        let dmabuf: zwp_linux_dmabuf_v1::ZwpLinuxDmabufV1 = self.bind("zwp_linux_dmabuf_v1", 3);
        let params = dmabuf.create_params(&self.handle, ());
        params.add(fd, 0, 0, (width * 4) as u32, 0, 0);
        let buffer = params.create_immed(
            width,
            height,
            ARGB8888,
            zwp_linux_buffer_params_v1::Flags::empty(),
            &self.handle,
            (),
        );
        window.surface.attach(Some(&buffer), 0, 0);
        window.surface.damage_buffer(0, 0, width, height);
        window.surface.commit();
        buffer
    }

    pub(crate) fn popup(
        &mut self,
        server: &mut Server,
        parent: &TestWindow,
        anchor: (i32, i32),
        size: (i32, i32),
    ) -> TestWindow {
        self.popup_grabbing(server, parent, anchor, size, None)
    }

    pub(crate) fn grabbing_popup(
        &mut self,
        server: &mut Server,
        parent: &TestWindow,
        anchor: (i32, i32),
        size: (i32, i32),
    ) -> TestWindow {
        let serial = self
            .received
            .serial
            .expect("a grab is taken with the serial of an input event");
        self.popup_grabbing(server, parent, anchor, size, Some(serial))
    }

    fn popup_grabbing(
        &mut self,
        server: &mut Server,
        parent: &TestWindow,
        anchor: (i32, i32),
        size: (i32, i32),
        grab: Option<u32>,
    ) -> TestWindow {
        let surface = self
            .compositor
            .as_ref()
            .unwrap()
            .create_surface(&self.handle, ());
        let wm_base = self.wm_base.as_ref().unwrap();
        let xdg_surface = wm_base.get_xdg_surface(&surface, &self.handle, ());
        let positioner = self.positioner(anchor, size);
        let popup = xdg_surface.get_popup(Some(&parent.xdg_surface), &positioner, &self.handle, ());
        positioner.destroy();
        if let Some(serial) = grab {
            popup.grab(self.seat.as_ref().unwrap(), serial);
        }
        surface.commit();
        self.exchange(server);
        let configured = self.received.configured.take();
        let window = TestWindow {
            surface,
            xdg_surface,
            toplevel: None,
            popup: Some(popup),
        };
        let Some(serial) = configured else {
            assert!(
                grab.is_some(),
                "a popup that does not grab is always configured"
            );
            return window;
        };
        window.xdg_surface.ack_configure(serial);
        self.attach(server, &window, size.0, size.1);
        window
    }

    fn positioner(&self, anchor: (i32, i32), size: (i32, i32)) -> xdg_positioner::XdgPositioner {
        let positioner = self
            .wm_base
            .as_ref()
            .unwrap()
            .create_positioner(&self.handle, ());
        positioner.set_size(size.0, size.1);
        positioner.set_anchor_rect(anchor.0, anchor.1, 1, 1);
        positioner.set_anchor(xdg_positioner::Anchor::TopLeft);
        positioner.set_gravity(xdg_positioner::Gravity::BottomRight);
        positioner.set_constraint_adjustment(xdg_positioner::ConstraintAdjustment::all());
        positioner
    }

    pub(crate) fn reposition(
        &mut self,
        server: &mut Server,
        popup: &TestWindow,
        anchor: (i32, i32),
        size: (i32, i32),
        token: u32,
    ) {
        let positioner = self.positioner(anchor, size);
        popup
            .popup
            .as_ref()
            .expect("only a popup is repositioned")
            .reposition(&positioner, token);
        positioner.destroy();
        self.exchange(server);
    }

    pub(crate) fn attach(
        &mut self,
        server: &mut Server,
        window: &TestWindow,
        width: i32,
        height: i32,
    ) {
        self.attach_unsent(window, width, height);
        self.exchange(server);
    }

    pub(crate) fn attach_unsent(&mut self, window: &TestWindow, width: i32, height: i32) {
        let length = (width * 4 * height) as usize;
        self.attach_pixels_unsent(window, width, height, &vec![0xff; length]);
    }

    pub(crate) fn attach_pixels_unsent(
        &mut self,
        window: &TestWindow,
        width: i32,
        height: i32,
        pixels: &[u8],
    ) {
        self.attach_surface_pixels(&window.surface, width, height, pixels);
    }

    pub(crate) fn surface(&self) -> wl_surface::WlSurface {
        self.compositor
            .as_ref()
            .unwrap()
            .create_surface(&self.handle, ())
    }

    pub(crate) fn attach_surface(
        &mut self,
        surface: &wl_surface::WlSurface,
        width: i32,
        height: i32,
    ) {
        let length = (width * 4 * height) as usize;
        self.attach_surface_pixels(surface, width, height, &vec![0xff; length]);
    }

    pub(crate) fn attach_surface_pixels(
        &mut self,
        surface: &wl_surface::WlSurface,
        width: i32,
        height: i32,
        pixels: &[u8],
    ) {
        let stride = width * 4;
        let length = (stride * height) as usize;
        assert_eq!(pixels.len(), length, "the pixels fill the buffer");
        let fd = rustix::fs::memfd_create("buffer", rustix::fs::MemfdFlags::CLOEXEC)
            .expect("a memfd opens");
        let mut file = std::fs::File::from(fd);
        file.write_all(pixels).expect("the buffer is written");
        let pool = self.shm.as_ref().unwrap().create_pool(
            std::os::fd::AsFd::as_fd(&file),
            length as i32,
            &self.handle,
            (),
        );
        let buffer = pool.create_buffer(
            0,
            width,
            height,
            stride,
            wl_shm::Format::Argb8888,
            &self.handle,
            (),
        );
        surface.attach(Some(&buffer), 0, 0);
        surface.damage_buffer(0, 0, width, height);
        surface.commit();
    }

    pub(crate) fn keyboard(&mut self) -> wl_keyboard::WlKeyboard {
        self.seat.as_ref().unwrap().get_keyboard(&self.handle, ())
    }

    pub(crate) fn pointer(&mut self) -> wl_pointer::WlPointer {
        self.seat.as_ref().unwrap().get_pointer(&self.handle, ())
    }
}

impl Dispatch<wl_registry::WlRegistry, ()> for Received {
    fn event(
        state: &mut Self,
        _: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            wl_registry::Event::Global {
                name,
                interface,
                version,
            } => state.globals.push((name, interface, version)),
            wl_registry::Event::GlobalRemove { name } => {
                state.globals.retain(|(held, _, _)| *held != name);
                state.removed.push(name);
                state.output_names.retain(|(held, _)| *held != name);
                state.entered.retain(|held| *held != name);
            }
            _ => {}
        }
    }
}

impl Dispatch<xdg_wm_base::XdgWmBase, ()> for Received {
    fn event(
        state: &mut Self,
        base: &xdg_wm_base::XdgWmBase,
        event: xdg_wm_base::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let xdg_wm_base::Event::Ping { serial } = event {
            state.pings.push(serial);
            if !state.ignores_pings {
                base.pong(serial);
            }
        }
    }
}

impl Dispatch<xdg_surface::XdgSurface, ()> for Received {
    fn event(
        state: &mut Self,
        _: &xdg_surface::XdgSurface,
        event: xdg_surface::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let xdg_surface::Event::Configure { serial } = event {
            state.configured = Some(serial);
        }
    }
}

impl Dispatch<xdg_toplevel::XdgToplevel, ()> for Received {
    fn event(
        state: &mut Self,
        _: &xdg_toplevel::XdgToplevel,
        event: xdg_toplevel::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let xdg_toplevel::Event::Configure {
            width,
            height,
            states,
        } = event
        {
            state.size = Some((width, height));
            let states: Vec<u32> = states
                .as_chunks::<4>()
                .0
                .iter()
                .map(|chunk| u32::from_ne_bytes(*chunk))
                .collect();
            let has = |which: xdg_toplevel::State| states.contains(&(which as u32));
            state.activated = has(xdg_toplevel::State::Activated);
            state.fullscreen = has(xdg_toplevel::State::Fullscreen);
            state.maximized = has(xdg_toplevel::State::Maximized);
        } else if let xdg_toplevel::Event::WmCapabilities { capabilities } = event {
            state.protocols.wm_capabilities = Some(
                capabilities
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .map(|chunk| u32::from_ne_bytes(*chunk))
                    .collect(),
            );
        }
    }
}

impl Dispatch<wl_keyboard::WlKeyboard, ()> for Received {
    fn event(
        state: &mut Self,
        _: &wl_keyboard::WlKeyboard,
        event: wl_keyboard::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            wl_keyboard::Event::Enter { surface, .. } => {
                state.keyboard_entered = true;
                state.keyboard_surface = Some(surface);
            }
            wl_keyboard::Event::Leave { .. } => {
                state.keyboard_entered = false;
                state.keyboard_surface = None;
            }
            wl_keyboard::Event::Key {
                key,
                state: WEnum::Value(pressed),
                serial,
                ..
            } => {
                let pressed = pressed == wl_keyboard::KeyState::Pressed;
                state.keys.push((key, pressed));
                if pressed {
                    state.serial = Some(serial);
                    if let Some(surface) = state.keyboard_surface.clone() {
                        state.seen.push(Seen::Key(surface, key));
                    }
                }
            }
            wl_keyboard::Event::Keymap { .. } => state.keymaps += 1,
            wl_keyboard::Event::RepeatInfo { rate, delay } => state.repeat = Some((rate, delay)),
            _ => {}
        }
    }
}

impl Dispatch<wl_pointer::WlPointer, ()> for Received {
    fn event(
        state: &mut Self,
        _: &wl_pointer::WlPointer,
        event: wl_pointer::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            wl_pointer::Event::Enter {
                serial,
                surface,
                surface_x,
                surface_y,
            } => {
                state.pointer_entered = Some((surface_x, surface_y));
                state.pointer_serial = Some(serial);
                state.pointer_surface = Some(surface);
            }
            wl_pointer::Event::Leave { .. } => state.pointer_surface = None,
            wl_pointer::Event::Button {
                button,
                state: WEnum::Value(pressed),
                serial,
                ..
            } => {
                let pressed = pressed == wl_pointer::ButtonState::Pressed;
                state.buttons.push((button, pressed));
                if pressed {
                    state.serial = Some(serial);
                }
                if let Some(surface) = state.pointer_surface.clone() {
                    state.seen.push(Seen::Button(surface, pressed));
                }
            }
            wl_pointer::Event::Axis { value, .. } => state.scrolled += value,
            _ => {}
        }
    }
}

impl Dispatch<wl_callback::WlCallback, ()> for Received {
    fn event(
        state: &mut Self,
        _: &wl_callback::WlCallback,
        event: wl_callback::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wl_callback::Event::Done { .. } = event {
            state.frames += 1;
        }
    }
}

wayland_client::delegate_noop!(Received: ignore wl_compositor::WlCompositor);
impl Dispatch<wl_output::WlOutput, u32> for Received {
    fn event(
        state: &mut Self,
        _: &wl_output::WlOutput,
        event: wl_output::Event,
        global: &u32,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wl_output::Event::Name { name } = event {
            state.output_names.retain(|(held, _)| held != global);
            state.output_names.push((*global, name));
        }
    }
}

impl Dispatch<wl_surface::WlSurface, ()> for Received {
    fn event(
        state: &mut Self,
        _: &wl_surface::WlSurface,
        event: wl_surface::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let output_name = |output: &wl_output::WlOutput| wayland_client::Proxy::data::<u32>(output).copied();
        match event {
            wl_surface::Event::Enter { output } => state.entered.extend(output_name(&output)),
            wl_surface::Event::Leave { output } => {
                let left = output_name(&output);
                state.entered.retain(|held| Some(*held) != left);
            }
            _ => {}
        }
    }
}
wayland_client::delegate_noop!(Received: ignore wl_shm::WlShm);
wayland_client::delegate_noop!(Received: ignore wl_shm_pool::WlShmPool);
impl Dispatch<wl_buffer::WlBuffer, ()> for Received {
    fn event(
        state: &mut Self,
        buffer: &wl_buffer::WlBuffer,
        event: wl_buffer::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wl_buffer::Event::Release = event {
            state.released.push(buffer.clone());
        }
    }
}

impl Dispatch<zxdg_toplevel_decoration_v1::ZxdgToplevelDecorationV1, ()> for Received {
    fn event(
        state: &mut Self,
        _: &zxdg_toplevel_decoration_v1::ZxdgToplevelDecorationV1,
        event: zxdg_toplevel_decoration_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let zxdg_toplevel_decoration_v1::Event::Configure {
            mode: WEnum::Value(mode),
        } = event
        {
            state.xdg_decoration = Some(mode);
        }
    }
}

impl Dispatch<org_kde_kwin_server_decoration_manager::OrgKdeKwinServerDecorationManager, ()>
    for Received
{
    fn event(
        state: &mut Self,
        _: &org_kde_kwin_server_decoration_manager::OrgKdeKwinServerDecorationManager,
        event: org_kde_kwin_server_decoration_manager::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let org_kde_kwin_server_decoration_manager::Event::DefaultMode {
            mode: WEnum::Value(mode),
        } = event
        {
            state.kde_default_decoration = Some(mode);
        }
    }
}

impl Dispatch<org_kde_kwin_server_decoration::OrgKdeKwinServerDecoration, ()> for Received {
    fn event(
        state: &mut Self,
        _: &org_kde_kwin_server_decoration::OrgKdeKwinServerDecoration,
        event: org_kde_kwin_server_decoration::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let org_kde_kwin_server_decoration::Event::Mode {
            mode: WEnum::Value(mode),
        } = event
        {
            state.kde_decoration = Some(mode);
        }
    }
}

wayland_client::delegate_noop!(Received: ignore zxdg_decoration_manager_v1::ZxdgDecorationManagerV1);
wayland_client::delegate_noop!(Received: ignore wl_seat::WlSeat);
wayland_client::delegate_noop!(Received: ignore zwp_idle_inhibit_manager_v1::ZwpIdleInhibitManagerV1);
wayland_client::delegate_noop!(Received: ignore zwp_idle_inhibitor_v1::ZwpIdleInhibitorV1);
wayland_client::delegate_noop!(Received: ignore ext_idle_notifier_v1::ExtIdleNotifierV1);
impl Dispatch<ext_idle_notification_v1::ExtIdleNotificationV1, ()> for Received {
    fn event(
        received: &mut Self,
        _notification: &ext_idle_notification_v1::ExtIdleNotificationV1,
        event: ext_idle_notification_v1::Event,
        _data: &(),
        _connection: &Connection,
        _handle: &QueueHandle<Self>,
    ) {
        match event {
            ext_idle_notification_v1::Event::Idled => received.idle.push(true),
            ext_idle_notification_v1::Event::Resumed => received.idle.push(false),
            _ => {}
        }
    }
}
wayland_client::delegate_noop!(Received: ignore zwp_linux_dmabuf_v1::ZwpLinuxDmabufV1);
wayland_client::delegate_noop!(Received: ignore zwp_linux_buffer_params_v1::ZwpLinuxBufferParamsV1);
wayland_client::delegate_noop!(Received: ignore xdg_positioner::XdgPositioner);
wayland_client::delegate_noop!(Received: ignore wayland_protocols::xdg::dialog::v1::client::xdg_wm_dialog_v1::XdgWmDialogV1);
wayland_client::delegate_noop!(Received: ignore wayland_protocols::xdg::dialog::v1::client::xdg_dialog_v1::XdgDialogV1);
wayland_client::delegate_noop!(Received: ignore wp_cursor_shape_manager_v1::WpCursorShapeManagerV1);
wayland_client::delegate_noop!(Received: ignore wp_cursor_shape_device_v1::WpCursorShapeDeviceV1);
impl Dispatch<xdg_popup::XdgPopup, ()> for Received {
    fn event(
        state: &mut Self,
        popup: &xdg_popup::XdgPopup,
        event: xdg_popup::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            xdg_popup::Event::PopupDone => state.seen.push(Seen::PopupDone(popup.clone())),
            xdg_popup::Event::Configure {
                x,
                y,
                width,
                height,
            } => state.popup_geometry = Some((x, y, width, height)),
            xdg_popup::Event::Repositioned { token } => state.repositioned = Some(token),
            _ => {}
        }
    }
}

pub(crate) use be_dmabuf::testing::{pattern, read, vulkan_device};

wayland_client::delegate_noop!(Received: ignore xdg_activation_v1::XdgActivationV1);
impl Dispatch<xdg_activation_token_v1::XdgActivationTokenV1, ()> for Received {
    fn event(
        received: &mut Self,
        _: &xdg_activation_token_v1::XdgActivationTokenV1,
        event: xdg_activation_token_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let xdg_activation_token_v1::Event::Done { token } = event {
            received.token = Some(token);
        }
    }
}

impl Dispatch<wp_presentation::WpPresentation, ()> for Received {
    fn event(
        state: &mut Self,
        _: &wp_presentation::WpPresentation,
        event: wp_presentation::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wp_presentation::Event::ClockId { clk_id } = event {
            state.clock = Some(clk_id);
        }
    }
}

impl Dispatch<wp_presentation_feedback::WpPresentationFeedback, ()> for Received {
    fn event(
        state: &mut Self,
        _: &wp_presentation_feedback::WpPresentationFeedback,
        event: wp_presentation_feedback::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            wp_presentation_feedback::Event::Presented {
                tv_sec_hi,
                tv_sec_lo,
                tv_nsec,
                refresh,
                seq_hi,
                seq_lo,
                flags,
            } => state.presented.push(Feedback {
                time: std::time::Duration::new(
                    (u64::from(tv_sec_hi) << 32) | u64::from(tv_sec_lo),
                    tv_nsec,
                ),
                refresh,
                sequence: (u64::from(seq_hi) << 32) | u64::from(seq_lo),
                flags: match flags {
                    WEnum::Value(kind) => kind.bits(),
                    WEnum::Unknown(bits) => bits,
                },
            }),
            wp_presentation_feedback::Event::Discarded => state.discarded += 1,
            _ => {}
        }
    }
}
