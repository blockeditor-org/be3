use std::io::Write as _;

use wayland_client::protocol::{wl_buffer, wl_compositor, wl_registry, wl_shm, wl_shm_pool, wl_surface};
use wayland_client::{Connection, Dispatch, QueueHandle, delegate_noop};
use wayland_protocols::xdg::shell::client::{xdg_surface, xdg_toplevel, xdg_wm_base};

/// The size it takes when the compositor lets it choose.
const CHOSEN: (i32, i32) = (320, 240);

/// Its color, as `wl_shm`'s little-endian XRGB8888: a mid blue.
const COLOR: [u8; 4] = [0xd0, 0x80, 0x30, 0xff];

#[derive(Default)]
struct State {
    compositor: Option<wl_compositor::WlCompositor>,
    shm: Option<wl_shm::WlShm>,
    wm_base: Option<xdg_wm_base::XdgWmBase>,
    /// The size the last configure asked for, until it is acknowledged and drawn.
    asked: Option<(i32, i32)>,
    /// The serial of the configure to acknowledge.
    serial: Option<u32>,
    closed: bool,
}

/// Opens the window and serves it until it is closed.
pub(super) fn run(title: &str) -> Result<(), String> {
    let connection = Connection::connect_to_env()
        .map_err(|error| format!("it found no Wayland compositor: {error}"))?;
    let mut queue = connection.new_event_queue();
    let handle = queue.handle();
    connection.display().get_registry(&handle, ());
    let mut state = State::default();
    queue
        .roundtrip(&mut state)
        .map_err(|error| format!("the compositor did not answer: {error}"))?;
    let (Some(compositor), Some(shm), Some(wm_base)) =
        (state.compositor.clone(), state.shm.clone(), state.wm_base.clone())
    else {
        return Err("the compositor offers no wl_compositor, wl_shm or xdg_wm_base".to_owned());
    };
    let surface = compositor.create_surface(&handle, ());
    let xdg = wm_base.get_xdg_surface(&surface, &handle, ());
    let toplevel = xdg.get_toplevel(&handle, ());
    toplevel.set_title(title.to_owned());
    toplevel.set_app_id("be-tool-wayland-window".to_owned());
    surface.commit();
    while !state.closed {
        queue
            .blocking_dispatch(&mut state)
            .map_err(|error| format!("the connection ended: {error}"))?;
        if let Some(serial) = state.serial.take() {
            xdg.ack_configure(serial);
            let (width, height) = match state.asked.take() {
                Some((width, height)) if width > 0 && height > 0 => (width, height),
                _ => CHOSEN,
            };
            draw(&surface, &shm, &handle, width, height)?;
        }
    }
    Ok(())
}

/// Attaches a buffer `width` by `height` filled with `COLOR` and commits it.
fn draw(
    surface: &wl_surface::WlSurface,
    shm: &wl_shm::WlShm,
    handle: &QueueHandle<State>,
    width: i32,
    height: i32,
) -> Result<(), String> {
    let stride = width * 4;
    let length = stride * height;
    let fd = rustix::fs::memfd_create("buffer", rustix::fs::MemfdFlags::CLOEXEC)
        .map_err(|error| format!("no memfd for its buffer: {error}"))?;
    let mut file = std::fs::File::from(fd);
    let pixels: Vec<u8> = COLOR.repeat((width * height) as usize);
    file.write_all(&pixels)
        .map_err(|error| format!("its buffer was not written: {error}"))?;
    let pool = shm.create_pool(std::os::fd::AsFd::as_fd(&file), length, handle, ());
    let buffer = pool.create_buffer(0, width, height, stride, wl_shm::Format::Xrgb8888, handle, ());
    pool.destroy();
    surface.attach(Some(&buffer), 0, 0);
    surface.damage_buffer(0, 0, width, height);
    surface.commit();
    Ok(())
}

impl Dispatch<wl_registry::WlRegistry, ()> for State {
    fn event(
        state: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        _: &(),
        _: &Connection,
        handle: &QueueHandle<Self>,
    ) {
        let wl_registry::Event::Global {
            name, interface, ..
        } = event
        else {
            return;
        };
        match interface.as_str() {
            "wl_compositor" => state.compositor = Some(registry.bind(name, 4, handle, ())),
            "wl_shm" => state.shm = Some(registry.bind(name, 1, handle, ())),
            "xdg_wm_base" => state.wm_base = Some(registry.bind(name, 1, handle, ())),
            _ => {}
        }
    }
}

impl Dispatch<xdg_wm_base::XdgWmBase, ()> for State {
    fn event(
        _: &mut Self,
        wm_base: &xdg_wm_base::XdgWmBase,
        event: xdg_wm_base::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let xdg_wm_base::Event::Ping { serial } = event {
            wm_base.pong(serial);
        }
    }
}

impl Dispatch<xdg_surface::XdgSurface, ()> for State {
    fn event(
        state: &mut Self,
        _: &xdg_surface::XdgSurface,
        event: xdg_surface::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let xdg_surface::Event::Configure { serial } = event {
            state.serial = Some(serial);
        }
    }
}

impl Dispatch<xdg_toplevel::XdgToplevel, ()> for State {
    fn event(
        state: &mut Self,
        _: &xdg_toplevel::XdgToplevel,
        event: xdg_toplevel::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            xdg_toplevel::Event::Configure { width, height, .. } => {
                state.asked = Some((width, height));
            }
            xdg_toplevel::Event::Close => state.closed = true,
            _ => {}
        }
    }
}

delegate_noop!(State: ignore wl_compositor::WlCompositor);
delegate_noop!(State: ignore wl_surface::WlSurface);
delegate_noop!(State: ignore wl_shm::WlShm);
delegate_noop!(State: ignore wl_shm_pool::WlShmPool);
impl Dispatch<wl_buffer::WlBuffer, ()> for State {
    fn event(
        _: &mut Self,
        buffer: &wl_buffer::WlBuffer,
        event: wl_buffer::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        // Each configure draws a buffer of its own: one the compositor let go of is done with.
        if let wl_buffer::Event::Release = event {
            buffer.destroy();
        }
    }
}
