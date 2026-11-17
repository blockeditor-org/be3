//! The window's own background on X11. Between the server growing a window and the program
//! drawing the new size, the server fills the new part with the window's background, which is
//! black unless set: setting it to the program's clear colour shows that colour there instead,
//! beside the last frame, which stays at the window's corner.
//! Other systems keep the last frame or a colour of their own meanwhile.

use std::cell::{Cell, RefCell};

use beui_types::Color32;
use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{ChangeWindowAttributesAux, ConnectionExt as _, Gravity};
use x11rb::rust_connection::RustConnection;

thread_local! {
    // A connection of its own: winit's is an Xlib or XCB pointer, which x11rb could only wrap
    // through its unsafe XCB bindings. Any client may change another's window attributes.
    static CONNECTION: RefCell<Option<Option<RustConnection>>> = const { RefCell::new(None) };
    /// The clear colour last set, for a window made after it (`fresh`).
    static COLOR: Cell<Option<Color32>> = const { Cell::new(None) };
}

/// Gives a window just made the clear colour set last, if any.
pub(crate) fn fresh(window: &dyn winit::window::Window) {
    if let Some(color) = COLOR.get() {
        set(window, color);
    }
}

/// Sets `window`'s X11 background to `color`; does nothing on any other system.
pub(crate) fn set(window: &dyn winit::window::Window, color: Color32) {
    COLOR.set(Some(color));
    let Ok(handle) = window.window_handle() else {
        return;
    };
    let id = match handle.as_raw() {
        RawWindowHandle::Xlib(handle) => handle.window as u32,
        RawWindowHandle::Xcb(handle) => handle.window.get(),
        _ => return,
    };
    // The pixel of a TrueColor visual, 24 or 32 bits deep, as winit's windows use: red, green
    // and blue bytes, with an opaque alpha that a 24-bit visual ignores.
    let [red, green, blue, _] = color.to_array();
    let pixel = 0xff00_0000 | (u32::from(red) << 16) | (u32::from(green) << 8) | u32::from(blue);
    CONNECTION.with(|connection| {
        let mut connection = connection.borrow_mut();
        let Some(connection) = connection
            .get_or_insert_with(|| x11rb::connect(None).ok().map(|(connection, _)| connection))
        else {
            return;
        };
        // North-west gravity keeps the last frame at the window's corner as it grows: with a
        // background and the default gravity, the server would clear all of it to that colour.
        let attributes = ChangeWindowAttributesAux::new()
            .background_pixel(pixel)
            .bit_gravity(Gravity::NORTH_WEST);
        if connection.change_window_attributes(id, &attributes).is_ok() {
            let _ = connection.flush();
        }
    });
}
