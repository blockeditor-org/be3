//! A root guest built on be-sdk-root, which be-wasm-root-host's tests and the host test
//! `a_root_in_place_of_the_hosts_ui_shows_the_editor_it_embeds` run
//! (plans/host_as_platform_adapter.md, steps 4e5b and 4g2). On its first frame it asks its host
//! for its fonts, spawns the triangle as a staged app (`APP`) and opens its editor with one
//! screen (`SCREEN`), painted at `RASTER` pixels per point; each frame it asks the app for a
//! frame when one is due, races that against the frame's deadline, and shows its own surface with
//! the triangle's screen over it at `ORIGIN`, by handle, once the app painted it, and
//! `LATE_LAYER` when the deadline came first.
//! Its own surface (`ROOT_SURFACE`) it draws with beui (be-sdk-beui-root): cleared to
//! `ROOT_COLOR`, with `TEXT` at its top left in the host's fonts, and `LATER_TEXT` in its place
//! from `LATER` on.
//! A frame at `CLOSES` lets go of the app, which stops it. The window's input is routed by the
//! library over the layers it drew last, and what lands on its own surface its beui document
//! hears.
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

/// The line of text the root's own surface shows, in the host's fonts.
pub const TEXT: &str = "The root draws this line with beui";

/// Where `TEXT` starts: its top left, in the window's points, in the strip above the screen.
pub const TEXT_ORIGIN: (f32, f32) = (3.0, 3.0);

/// The size of `TEXT`'s font, in points: small enough that its line stays above the screen.
pub const TEXT_SIZE: f32 = 12.0;

/// How long after its first frame, by its host's frame clock, the root's document shows
/// `LATER_TEXT` in place of `TEXT`: a timer of its own, which only a frame drawn when it is due
/// shows.
pub const LATER: std::time::Duration = std::time::Duration::from_secs(1);

/// What the root's own surface shows from `LATER` on.
pub const LATER_TEXT: &str = "The root draws this line with beui, and redraws it on time";

#[cfg(target_arch = "wasm32")]
mod guest;
