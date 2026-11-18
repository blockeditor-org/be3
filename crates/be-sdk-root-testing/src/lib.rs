//! A root guest built on be-sdk-root, which be-wasm-root-host's tests and the host test
//! `a_root_in_place_of_the_hosts_ui_shows_the_editor_it_embeds` run
//! (plans/host_as_platform_adapter.md, steps 4e5b and 4g2). On its first frame it asks its host
//! for its fonts, spawns the triangle as a staged app (`APP`) and opens its editor with one
//! screen (`SCREEN`), painted at `RASTER` pixels per point; each frame it asks the app for a
//! frame when one is due, races that against the frame's deadline, and shows its own surface with
//! the triangle's screen over it at `ORIGIN`, by handle, once the app painted it, and
//! `LATE_LAYER` when the deadline came first.
//! Its own surface (`ROOT_SURFACE`) it fills with `ROOT_COLOR` through `be3:gpu`; over it, the
//! strip `TEXT_SURFACE` it draws with beui (be-sdk-beui-root): `TEXT` in the host's fonts, and
//! `LATER_TEXT` in its place from `LATER` on.
//! It tells the app the fonts its host gave. Over everything it shows its host's notices
//! (`be3:root`'s `host.notices`) as toasts, drawn with beui on `TOAST_SURFACE` at the window's
//! top right and shown by `TOAST_LAYER` while any show, hit only where they are and never
//! taking the focus; what the person answers it tells its host (`answer-notice`).
//! A frame at `CLOSES` lets go of the app, which stops it. The window's input is routed by the
//! library over the layers it drew last; what lands on `TEXT_SURFACE` its beui document hears,
//! and what lands on its own surface beside the strip nothing does.
//!
//! The triangle needs no block content (its `connect` reads nothing), so the root serves it no
//! block store: that waits for step 5.
//!
//! Only the guest (`guest`, wasm32 alone) needs more than the constants below, which the tests
//! that run it read natively.

use be_lib_layers::LayerId;
use be_sdk_api::ScreenId;

/// The staged app it spawns: the triangle's, by its manifest's id.
pub const APP: &str = "be3.triangle";

/// The triangle's entry point, as its manifest names it.
pub const APP_ENTRY: &str = "triangle.wasm";

/// The one screen the root asks of the editor.
pub const SCREEN: ScreenId = ScreenId(1);

/// The screen's size, in points.
pub const SCREEN_SIZE: (u32, u32) = (320, 200);

/// The pixels per point the screen is laid out and painted at.
pub const RASTER: u32 = 2;

/// Where the screen's top left sits in the window.
pub const ORIGIN: (f32, f32) = (40.0, 20.0);

/// The window's size, in points.
pub const SIZE: (u32, u32) = (400, 240);

/// The root's own surface, under the screen.
pub const ROOT_LAYER: LayerId = LayerId(0);

/// The id of the root's own surface, which `ROOT_LAYER` shows: it fills the window, a pixel to
/// a point.
pub const ROOT_SURFACE: u32 = 0;

/// What the root fills its own surface with, sRGB-encoded: a dull purple, unlike any window's
/// background or the triangle's colours.
pub const ROOT_COLOR: [f64; 4] = [0.3, 0.1, 0.4, 1.0];

/// Shown over the rest when the frame's deadline passed before the app answered.
pub const LATE_LAYER: LayerId = LayerId(1);

/// A frame at this time lets go of the app, which stops it, and shows the root's surface alone.
pub const CLOSES: u64 = u64::MAX;

/// A frame at this time makes the root's own surface `RESIZED` without presenting it again, and
/// configures `UNPRESENTED_SURFACE`, which it never presents, and shows the root's surface
/// alone, the app kept.
pub const RESIZES: u64 = u64::MAX - 1;

/// The size a `RESIZES` frame gives the root's own surface, in pixels.
pub const RESIZED: (u32, u32) = (200, 120);

/// A surface of the root's own that a `RESIZES` frame configures and never presents.
pub const UNPRESENTED_SURFACE: u32 = 2;

/// The layer showing `TEXT_SURFACE`, over the root's own surface.
pub const TEXT_LAYER: LayerId = LayerId(2);

/// A surface of the root's own that beui draws: the strip above the triangle's screen, at the
/// window's top left, clear but for its line of text, in the host's fonts.
pub const TEXT_SURFACE: u32 = 3;

/// `TEXT_SURFACE`'s size, a pixel to a point: as wide as the root's surface and as tall as the
/// strip above the screen.
pub const TEXT_STRIP: (u32, u32) = (400, 20);

/// The line of text `TEXT_SURFACE` shows.
pub const TEXT: &str = "The root draws this line with beui";

/// How long after its first frame, by its host's frame clock, the root's text shows
/// `LATER_TEXT` in place of `TEXT`: a timer of its own, which only a frame drawn when it is due
/// shows. An hour, so a host test, whose clock runs from the host's start until the test stops
/// it, never reaches it.
pub const LATER: std::time::Duration = std::time::Duration::from_secs(60 * 60);

/// What `TEXT_SURFACE` shows from `LATER` on.
pub const LATER_TEXT: &str = "The root draws this line with beui, and redraws it on time";

/// The layer showing `TOAST_SURFACE`, over everything else, while a toast shows.
pub const TOAST_LAYER: LayerId = LayerId(3);

/// A surface of the root's own that beui draws its host's notices on, as toasts at its top
/// right: clear but for them.
pub const TOAST_SURFACE: u32 = 4;

/// `TOAST_SURFACE`'s size, a pixel to a point, at the window's top left: as tall as the root's
/// surface, and wider, so a toast and its margins fit.
pub const TOAST_AREA: (u32, u32) = (480, 240);

#[cfg(target_arch = "wasm32")]
mod guest;
