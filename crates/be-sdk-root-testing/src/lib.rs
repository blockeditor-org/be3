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
//! F2 (`OPENS_DIALOG`), which the root always intercepts, opens a dialog of its own: a scrim
//! (`SCRIM_LAYER`, its own surface filled with `SCRIM_COLOR`) over everything but its toasts,
//! hit nowhere and capturing presses for the root's own layer, and over it the dialog
//! (`DIALOG_LAYER`), drawn with beui on `DIALOG_SURFACE`: a title, a name field it focuses and
//! Cancel and Rename. Once drawn, the dialog takes the focus, and the root hands its host the
//! field's input method area; it closes on a press on the scrim, on Escape (its document's while
//! it has the focus, the root's, which intercepts it, while it is open and the focus is elsewhere),
//! or on its own actions, and gives the focus back to the layer that
//! had it. Rename shows the name in the strip of text in place of `TEXT`. Its documents copy to
//! and paste from their host's clipboard (`be3:root`'s `host.copy` and `host.paste`).
//! F3 (`OPENS_STATISTICS`), which it always intercepts too, opens and closes a debug panel
//! (`STATISTICS_LAYER`, drawn with beui on `STATISTICS_SURFACE`) of its host's statistics: it
//! watches them while the panel is open (`be3:root`'s `host.watch-statistics`) and shows each
//! change of them (`host.statistics`) as rows, never taking the focus.
//! It shows its host's screen lock (`host.lock`): while the session is locked, a lock screen
//! (`LOCK_LAYER`, drawn with beui on `LOCK_SURFACE`) over everything, toasts hidden, which takes
//! the focus for its password field and gives it back once unlocked; it intercepts none of its
//! own keys then, only the F5 test fixture (`UNLOCKS_TWICE`).
//! Its field hands what was typed to the host to check (`host.unlock`), cleared as it does, and
//! shows why the host refused it. F4 (`LOCKS_SESSION`) asks the host to lock the session.
//! It shows whether its host's changes are saved (`host.saving`): while they stay unsaved for
//! `SLOW_SAVE`, the slow save's toast among its toasts, until they are saved or it is
//! dismissed; and while its core asks to keep display settings that just changed, that question
//! in its dialog's place (`KEEP_DISPLAY_TITLE`), counting down to reverting them, answered
//! through `host.core-command`, which Escape or a press on the scrim answers with revert.
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
use be_lib_ui_types::Key;
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

/// The key that opens the root's dialog, pressed with no modifiers: the root intercepts it
/// wherever the focus is.
pub const OPENS_DIALOG: Key = Key::F2;

/// The layer showing `SCRIM_SURFACE` while the dialog is open: over everything but the toasts,
/// hit nowhere and capturing every press that misses the dialog for `ROOT_LAYER`, its placer,
/// which closes the dialog.
pub const SCRIM_LAYER: LayerId = LayerId(4);

/// A surface of the root's own the size of the window, filled with `SCRIM_COLOR`.
pub const SCRIM_SURFACE: u32 = 5;

/// What the scrim is filled with: black, half covering what lies under it.
pub const SCRIM_COLOR: [f64; 4] = [0.0, 0.0, 0.0, 0.5];

/// The layer showing `DIALOG_SURFACE` while the dialog is open, over its scrim.
pub const DIALOG_LAYER: LayerId = LayerId(5);

/// A surface of the root's own that beui draws its dialog on, filling it, a pixel to a point.
pub const DIALOG_SURFACE: u32 = 6;

/// `DIALOG_SURFACE`'s size, a pixel to a point: as tall as its title, field and actions need.
pub const DIALOG_SIZE: (u32, u32) = (320, 152);

/// Where the dialog's top left sits in the window: centred in it.
pub const DIALOG_ORIGIN: (f32, f32) = (
    (SIZE.0 - DIALOG_SIZE.0) as f32 / 2.0,
    (SIZE.1 - DIALOG_SIZE.1) as f32 / 2.0,
);

/// The dialog's title.
pub const DIALOG_TITLE: &str = "Rename";

/// The title of the question of keeping display settings that just changed, which the root
/// asks in its dialog's place while its host asks it (`host.displays`).
pub const KEEP_DISPLAY_TITLE: &str = "Keep these display settings?";

/// The key that opens the root's statistics panel, and closes it again, pressed with no
/// modifiers: the root intercepts it wherever the focus is.
pub const OPENS_STATISTICS: Key = Key::F3;

/// The layer showing `STATISTICS_SURFACE` while the panel is open: over everything but the
/// dialog and the toasts, taking presses (it scrolls) but never the focus.
pub const STATISTICS_LAYER: LayerId = LayerId(6);

/// A surface of the root's own that beui draws the host's statistics on, as its debug panels
/// show them: its performance and memory rows.
pub const STATISTICS_SURFACE: u32 = 7;

/// `STATISTICS_SURFACE`'s size, a pixel to a point, at the window's top right.
pub const STATISTICS_AREA: (u32, u32) = (320, 240);

/// Where the panel's top left sits in the window: at its right edge.
pub const STATISTICS_ORIGIN: (f32, f32) = ((SIZE.0 - STATISTICS_AREA.0) as f32, 0.0);

/// The panel's title.
pub const STATISTICS_TITLE: &str = "Performance";

/// The key that asks the host to lock the session (`be3:root`'s `host.lock-session`), pressed
/// with no modifiers: the root intercepts it wherever the focus is, but not while the session is
/// locked.
pub const LOCKS_SESSION: Key = Key::F4;

/// A key that makes the root hand its host `STRAY_PASSWORD` twice at once (`host.unlock`),
/// whatever the lock says, as a root gone wrong might: what the tests check the host's guards
/// against. The root intercepts it always, while locked too.
pub const UNLOCKS_TWICE: Key = Key::F5;

/// What `UNLOCKS_TWICE` hands over.
pub const STRAY_PASSWORD: &str = "stray";

/// The layer showing `LOCK_SURFACE` while the session is locked: over everything, toasts too,
/// which it hides, taking every press and the focus.
pub const LOCK_LAYER: LayerId = LayerId(7);

/// A surface of the root's own the size of the window that beui draws its lock screen on.
pub const LOCK_SURFACE: u32 = 8;

/// How long its host's changes stay unsaved, by the host's frame clock, before the root shows
/// the slow save's toast, as be-host's own UI does.
pub const SLOW_SAVE: std::time::Duration = std::time::Duration::from_secs(4);

/// The slow save's toast's id among the toasts, which no notice of the host's has.
pub const SLOW_SAVE_TOAST: u64 = u64::MAX;

/// The slow save's toast's title.
pub const SLOW_SAVE_TITLE: &str = "Saving changes…";

/// The key that opens the root's list of the block stack's unsent edits (`host.core`), in its
/// dialog's place, pressed with no modifiers: the root intercepts it wherever the focus is, but
/// not while the session is locked.
pub const OPENS_UNSENT: Key = Key::F7;

/// The title of the list of unsent edits.
pub const UNSENT_TITLE: &str = "Unsent edits";

/// The title of the question the list asks before discarding a block's unsent edits.
pub const DISCARD_UNSENT_TITLE: &str = "Discard edits?";

/// The layer showing `ACCOUNTS_SURFACE` while the block stack waits for the person to open or
/// add an account (`core-view`'s `choosing-account`): over the app and the statistics panel,
/// under the dialog, the lock screen and the toasts, taking every press and, once drawn, the
/// focus.
pub const ACCOUNTS_LAYER: LayerId = LayerId(8);

/// A surface of the root's own the size of the window that beui draws the accounts on, as
/// be-host-accounts-ui's `AccountsScreen`.
pub const ACCOUNTS_SURFACE: u32 = 9;

/// The title of the dialog that adds an account, before it knows where.
pub const ADD_ACCOUNT_TITLE: &str = "Add account";

/// The title of the dialog while the person signs in in their browser.
pub const SIGN_IN_TITLE: &str = "Sign in with your browser";

/// The title of the question the root asks before removing an account kept on this device
/// alone, whose blocks go with it.
pub const FORGET_ACCOUNT_TITLE: &str = "Remove account?";

#[cfg(target_arch = "wasm32")]
mod guest;
