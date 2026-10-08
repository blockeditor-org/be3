use std::io::Write as _;
use std::os::unix::net::UnixStream;

use wayland_client::protocol::{
    wl_buffer, wl_callback, wl_compositor, wl_keyboard, wl_pointer, wl_registry, wl_seat, wl_shm,
    wl_shm_pool, wl_surface,
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
    pub(crate) buttons: Vec<(u32, bool)>,
    pub(crate) scrolled: f64,
    pub(crate) frames: usize,
    pub(crate) released: Vec<wl_buffer::WlBuffer>,
    pub(crate) xdg_decoration: Option<zxdg_toplevel_decoration_v1::Mode>,
    pub(crate) kde_default_decoration: Option<org_kde_kwin_server_decoration_manager::Mode>,
    pub(crate) kde_decoration: Option<org_kde_kwin_server_decoration::Mode>,
    pub(crate) ignores_pings: bool,
    pub(crate) pings: Vec<u32>,
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
        let positioner = wm_base.create_positioner(&self.handle, ());
        positioner.set_size(size.0, size.1);
        positioner.set_anchor_rect(anchor.0, anchor.1, 1, 1);
        positioner.set_anchor(xdg_positioner::Anchor::TopLeft);
        positioner.set_gravity(xdg_positioner::Gravity::BottomRight);
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
        let stride = width * 4;
        let length = (stride * height) as usize;
        let fd = rustix::fs::memfd_create("buffer", rustix::fs::MemfdFlags::CLOEXEC)
            .expect("a memfd opens");
        let mut file = std::fs::File::from(fd);
        file.write_all(&vec![0xff; length])
            .expect("the buffer is written");
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
        window.surface.attach(Some(&buffer), 0, 0);
        window.surface.damage_buffer(0, 0, width, height);
        window.surface.commit();
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
        if let wl_registry::Event::Global {
            name,
            interface,
            version,
        } = event
        {
            state.globals.push((name, interface, version));
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
                surface,
                surface_x,
                surface_y,
                ..
            } => {
                state.pointer_entered = Some((surface_x, surface_y));
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
wayland_client::delegate_noop!(Received: ignore wl_surface::WlSurface);
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
wayland_client::delegate_noop!(Received: ignore zwp_linux_dmabuf_v1::ZwpLinuxDmabufV1);
wayland_client::delegate_noop!(Received: ignore zwp_linux_buffer_params_v1::ZwpLinuxBufferParamsV1);
wayland_client::delegate_noop!(Received: ignore xdg_positioner::XdgPositioner);
impl Dispatch<xdg_popup::XdgPopup, ()> for Received {
    fn event(
        state: &mut Self,
        popup: &xdg_popup::XdgPopup,
        event: xdg_popup::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let xdg_popup::Event::PopupDone = event {
            state.seen.push(Seen::PopupDone(popup.clone()));
        }
    }
}

pub(crate) use be_dmabuf::testing::{pattern, read, vulkan_device};
