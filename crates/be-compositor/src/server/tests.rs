use super::*;

mod a_committed_buffer_becomes_a_layer_of_its_window;
mod a_title_reaches_the_ui;
mod configure_sends_the_panel_size;
mod destroying_a_toplevel_closes_its_window;
mod frame_callbacks_wait_for_send_frames;
mod keys_reach_the_focused_window;
mod the_pointer_enters_the_surface_under_it;

use std::io::Write as _;
use std::os::unix::net::UnixStream;

use wayland_client::protocol::{
    wl_buffer, wl_callback, wl_compositor, wl_keyboard, wl_pointer, wl_registry, wl_seat, wl_shm,
    wl_shm_pool, wl_surface,
};
use wayland_client::{Connection, Dispatch, EventQueue, QueueHandle, WEnum};
use wayland_protocols::xdg::shell::client::{xdg_surface, xdg_toplevel, xdg_wm_base};

use crate::state::{ServerEvent, WindowId};

#[derive(Default)]
struct Received {
    globals: Vec<(u32, String, u32)>,
    configured: Option<u32>,
    size: Option<(i32, i32)>,
    activated: bool,
    keyboard_entered: bool,
    keys: Vec<(u32, bool)>,
    pointer_entered: Option<(f64, f64)>,
    buttons: Vec<(u32, bool)>,
    frames: usize,
}

struct TestClient {
    connection: Connection,
    queue: EventQueue<Received>,
    handle: QueueHandle<Received>,
    received: Received,
    registry: wl_registry::WlRegistry,
    compositor: Option<wl_compositor::WlCompositor>,
    shm: Option<wl_shm::WlShm>,
    wm_base: Option<xdg_wm_base::XdgWmBase>,
    seat: Option<wl_seat::WlSeat>,
}

struct TestWindow {
    surface: wl_surface::WlSurface,
    xdg_surface: xdg_surface::XdgSurface,
    toplevel: xdg_toplevel::XdgToplevel,
}

fn server() -> Server {
    Server::headless().expect("a headless display starts")
}

impl TestClient {
    fn connect(server: &mut Server) -> Self {
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

    fn bind<I>(&self, interface: &str, version: u32) -> I
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

    fn exchange(&mut self, server: &mut Server) {
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

    fn open(&mut self, server: &mut Server) -> (TestWindow, WindowId) {
        let surface = self
            .compositor
            .as_ref()
            .unwrap()
            .create_surface(&self.handle, ());
        let xdg_surface = self
            .wm_base
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
                toplevel,
            },
            id,
        )
    }

    fn attach(&mut self, server: &mut Server, window: &TestWindow, width: i32, height: i32) {
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
        self.exchange(server);
    }

    fn keyboard(&mut self) -> wl_keyboard::WlKeyboard {
        self.seat.as_ref().unwrap().get_keyboard(&self.handle, ())
    }

    fn pointer(&mut self) -> wl_pointer::WlPointer {
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
        _: &mut Self,
        base: &xdg_wm_base::XdgWmBase,
        event: xdg_wm_base::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let xdg_wm_base::Event::Ping { serial } = event {
            base.pong(serial);
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
            state.activated = states
                .chunks_exact(4)
                .map(|chunk| u32::from_ne_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
                .any(|value| value == xdg_toplevel::State::Activated as u32);
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
            wl_keyboard::Event::Enter { .. } => state.keyboard_entered = true,
            wl_keyboard::Event::Leave { .. } => state.keyboard_entered = false,
            wl_keyboard::Event::Key {
                key,
                state: WEnum::Value(pressed),
                ..
            } => state
                .keys
                .push((key, pressed == wl_keyboard::KeyState::Pressed)),
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
                surface_x,
                surface_y,
                ..
            } => state.pointer_entered = Some((surface_x, surface_y)),
            wl_pointer::Event::Button {
                button,
                state: WEnum::Value(pressed),
                ..
            } => state
                .buttons
                .push((button, pressed == wl_pointer::ButtonState::Pressed)),
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
wayland_client::delegate_noop!(Received: ignore wl_buffer::WlBuffer);
wayland_client::delegate_noop!(Received: ignore wl_seat::WlSeat);
