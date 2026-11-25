use std::io::Write as _;

use wayland_client::protocol::{
    wl_buffer, wl_compositor, wl_keyboard, wl_pointer, wl_registry, wl_seat, wl_shm, wl_shm_pool,
    wl_surface,
};
use wayland_client::{Connection, Dispatch, QueueHandle, WEnum, delegate_noop};
use wayland_protocols::xdg::shell::client::{xdg_surface, xdg_toplevel, xdg_wm_base};

/// The size it takes when the compositor lets it choose.
const CHOSEN: (i32, i32) = (320, 240);

/// Its app id, which names it in the desktop's windows bar.
const APP_ID: &str = "be-tool-wayland-window";

/// Its color, as `wl_shm`'s little-endian XRGB8888: a mid blue.
const COLOR: [u8; 4] = [0xd0, 0x80, 0x30, 0xff];

/// Its color once a pointer pressed it: a green.
const PRESSED_COLOR: [u8; 4] = [0x40, 0xb0, 0x40, 0xff];

/// Its color once a key was pressed while it had the keyboard: a red.
const TYPED_COLOR: [u8; 4] = [0x30, 0x40, 0xd0, 0xff];

/// What reached it last, which it shows in its color and says in its app id, so a test sees the
/// window hear its input under the root, which names windows by their app ids.
#[derive(Clone, Copy, Default, PartialEq)]
enum Heard {
    #[default]
    Nothing,
    Pressed,
    Typed,
}

impl Heard {
    fn color(self) -> [u8; 4] {
        match self {
            Self::Nothing => COLOR,
            Self::Pressed => PRESSED_COLOR,
            Self::Typed => TYPED_COLOR,
        }
    }

    fn app_id(self) -> String {
        match self {
            Self::Nothing => APP_ID.to_owned(),
            Self::Pressed => format!("{APP_ID}.pressed"),
            Self::Typed => format!("{APP_ID}.typed"),
        }
    }
}

#[derive(Default)]
struct State {
    compositor: Option<wl_compositor::WlCompositor>,
    shm: Option<wl_shm::WlShm>,
    wm_base: Option<xdg_wm_base::XdgWmBase>,
    /// The size the last configure asked for, until it is acknowledged and drawn.
    asked: Option<(i32, i32)>,
    /// The serial of the configure to acknowledge.
    serial: Option<u32>,
    /// The size it drew last.
    size: Option<(i32, i32)>,
    heard: Heard,
    /// What it heard since it last drew.
    changed: bool,
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
    let (Some(compositor), Some(shm), Some(wm_base)) = (
        state.compositor.clone(),
        state.shm.clone(),
        state.wm_base.clone(),
    ) else {
        return Err("the compositor offers no wl_compositor, wl_shm or xdg_wm_base".to_owned());
    };
    let surface = compositor.create_surface(&handle, ());
    let xdg = wm_base.get_xdg_surface(&surface, &handle, ());
    let toplevel = xdg.get_toplevel(&handle, ());
    toplevel.set_title(title.to_owned());
    toplevel.set_app_id(APP_ID.to_owned());
    surface.commit();
    while !state.closed {
        queue
            .blocking_dispatch(&mut state)
            .map_err(|error| format!("the connection ended: {error}"))?;
        if let Some(serial) = state.serial.take() {
            xdg.ack_configure(serial);
            let (width, height) = match state.asked.take() {
                Some((width, height)) if width > 0 && height > 0 => (width, height),
                _ => state.size.unwrap_or(CHOSEN),
            };
            state.size = Some((width, height));
            draw(&surface, &shm, &handle, (width, height), state.heard.color())?;
        }
        // What it heard says itself in its app id, and in its color unless the configure it
        // took in with it drew that already.
        if std::mem::take(&mut state.changed)
            && let Some(size) = state.size
        {
            toplevel.set_app_id(state.heard.app_id());
            draw(&surface, &shm, &handle, size, state.heard.color())?;
        }
    }
    Ok(())
}

/// Attaches a buffer `size` big filled with `color` and commits it.
fn draw(
    surface: &wl_surface::WlSurface,
    shm: &wl_shm::WlShm,
    handle: &QueueHandle<State>,
    (width, height): (i32, i32),
    color: [u8; 4],
) -> Result<(), String> {
    let stride = width * 4;
    let length = stride * height;
    let fd = rustix::fs::memfd_create("buffer", rustix::fs::MemfdFlags::CLOEXEC)
        .map_err(|error| format!("no memfd for its buffer: {error}"))?;
    let mut file = std::fs::File::from(fd);
    let pixels: Vec<u8> = color.repeat((width * height) as usize);
    file.write_all(&pixels)
        .map_err(|error| format!("its buffer was not written: {error}"))?;
    let pool = shm.create_pool(std::os::fd::AsFd::as_fd(&file), length, handle, ());
    let buffer = pool.create_buffer(
        0,
        width,
        height,
        stride,
        wl_shm::Format::Xrgb8888,
        handle,
        (),
    );
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
            "wl_seat" => {
                registry.bind::<wl_seat::WlSeat, _, _>(name, 1, handle, ());
            }
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

impl Dispatch<wl_seat::WlSeat, ()> for State {
    fn event(
        _: &mut Self,
        seat: &wl_seat::WlSeat,
        event: wl_seat::Event,
        _: &(),
        _: &Connection,
        handle: &QueueHandle<Self>,
    ) {
        if let wl_seat::Event::Capabilities {
            capabilities: WEnum::Value(capabilities),
        } = event
        {
            if capabilities.contains(wl_seat::Capability::Pointer) {
                seat.get_pointer(handle, ());
            }
            if capabilities.contains(wl_seat::Capability::Keyboard) {
                seat.get_keyboard(handle, ());
            }
        }
    }
}

impl Dispatch<wl_pointer::WlPointer, ()> for State {
    fn event(
        state: &mut Self,
        _: &wl_pointer::WlPointer,
        event: wl_pointer::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wl_pointer::Event::Button {
            state: WEnum::Value(wl_pointer::ButtonState::Pressed),
            ..
        } = event
        {
            state.heard = Heard::Pressed;
            state.changed = true;
        }
    }
}

impl Dispatch<wl_keyboard::WlKeyboard, ()> for State {
    fn event(
        state: &mut Self,
        _: &wl_keyboard::WlKeyboard,
        event: wl_keyboard::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        // The keymap's file closes as the event drops: it types nothing, so it needs none.
        if let wl_keyboard::Event::Key {
            state: WEnum::Value(wl_keyboard::KeyState::Pressed),
            ..
        } = event
        {
            state.heard = Heard::Typed;
            state.changed = true;
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
