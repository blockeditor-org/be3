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

use wayland_protocols::wp::linux_dmabuf::zv1::client::{
    zwp_linux_buffer_params_v1, zwp_linux_dmabuf_v1,
};

const ARGB8888: u32 = u32::from_le_bytes(*b"AR24");
use crate::state::{ServerEvent, WindowId};

#[derive(Default)]
pub(crate) struct Received {
    pub(crate) globals: Vec<(u32, String, u32)>,
    pub(crate) configured: Option<u32>,
    pub(crate) size: Option<(i32, i32)>,
    pub(crate) activated: bool,
    pub(crate) keyboard_entered: bool,
    pub(crate) keys: Vec<(u32, bool)>,
    pub(crate) pointer_entered: Option<(f64, f64)>,
    pub(crate) pointer_surface: Option<wl_surface::WlSurface>,
    pub(crate) buttons: Vec<(u32, bool)>,
    pub(crate) frames: usize,
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
    pub(crate) _popup: Option<xdg_popup::XdgPopup>,
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
                _popup: None,
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
            _popup: None,
        }
    }

    pub(crate) fn flush(&mut self) {
        self.connection.flush().expect("the client flushes");
    }

    pub(crate) fn receive(&mut self) {
        if let Some(guard) = self.queue.prepare_read() {
            let _ = guard.read();
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
    ) {
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
    }

    pub(crate) fn popup(
        &mut self,
        server: &mut Server,
        parent: &TestWindow,
        anchor: (i32, i32),
        size: (i32, i32),
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
        surface.commit();
        self.exchange(server);
        let serial = self
            .received
            .configured
            .take()
            .expect("the popup's first commit is configured");
        xdg_surface.ack_configure(serial);
        let window = TestWindow {
            surface,
            xdg_surface,
            toplevel: None,
            _popup: Some(popup),
        };
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
                .as_chunks::<4>()
                .0
                .iter()
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
                surface,
                surface_x,
                surface_y,
                ..
            } => {
                state.pointer_entered = Some((surface_x, surface_y));
                state.pointer_surface = Some(surface);
            }
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
wayland_client::delegate_noop!(Received: ignore zwp_linux_dmabuf_v1::ZwpLinuxDmabufV1);
wayland_client::delegate_noop!(Received: ignore zwp_linux_buffer_params_v1::ZwpLinuxBufferParamsV1);
wayland_client::delegate_noop!(Received: ignore xdg_positioner::XdgPositioner);
wayland_client::delegate_noop!(Received: ignore xdg_popup::XdgPopup);

pub(crate) fn vulkan_device() -> (wgpu::Device, wgpu::Queue) {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: wgpu::Backends::VULKAN,
        ..wgpu::InstanceDescriptor::new_without_display_handle()
    });
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::LowPower,
        force_fallback_adapter: false,
        compatible_surface: None,
    }))
    .expect("a Vulkan adapter is available");
    crate::gpu::open_device(
        &adapter,
        &wgpu::DeviceDescriptor {
            label: Some("compositor test device"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::downlevel_defaults(),
            experimental_features: wgpu::ExperimentalFeatures::disabled(),
            memory_hints: wgpu::MemoryHints::Performance,
            trace: wgpu::Trace::Off,
        },
    )
    .expect("the Vulkan device opens")
}

pub(crate) fn pattern(width: u32, height: u32) -> Vec<u8> {
    (0..width * height)
        .flat_map(|index| [(index % 251) as u8, (index / 251 % 251) as u8, 90, 255])
        .collect()
}

pub(crate) fn read(device: &wgpu::Device, queue: &wgpu::Queue, texture: &wgpu::Texture) -> Vec<u8> {
    let size = texture.size();
    let row = size.width * 4;
    let padded = row.div_ceil(256) * 256;
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("readback"),
        size: u64::from(padded * size.height),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    encoder.copy_texture_to_buffer(
        texture.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(padded),
                rows_per_image: Some(size.height),
            },
        },
        size,
    );
    queue.submit([encoder.finish()]);
    buffer.slice(..).map_async(wgpu::MapMode::Read, |result| {
        result.expect("the readback maps");
    });
    device
        .poll(wgpu::PollType::wait_indefinitely())
        .expect("the device finishes");
    let mapped = buffer.slice(..).get_mapped_range();
    mapped
        .chunks(padded as usize)
        .flat_map(|chunk| chunk[..row as usize].to_vec())
        .collect()
}
