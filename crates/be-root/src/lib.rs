//! The root be-host runs in place of its own UI (plans/host_as_platform_adapter.md, 4g7a;
//! decisions/the_real_root_is_its_own_crate_not_the_testing_root.md), built on be-sdk-root and
//! drawn with beui through be-sdk-beui-root, worded and laid out as be-host's own UI and filling
//! the window `frame-request` tells it of. It shows what `be3:root` carries of the host:
//! - its notices as toasts (`TOAST_LAYER`), with the slow save's toast while changes stay unsaved
//!   (`host.saving`);
//! - the session's lock screen while locked (`LOCK_LAYER`, `host.lock`), over everything;
//! - what the block stack waits for before an organization's blocks open, on its onboarding
//!   (`ONBOARDING_LAYER`, `host.core`'s `waits-for`): the accounts, the organizations, the
//!   sessions, the recovery phrase, unlocking, and a failure;
//! - the host's questions in its dialog's place, be-host's own dialog over beui's scrim
//!   (`DIALOG_LAYER`):
//!   keeping display settings (`host.displays`), discarding unsaved changes, signing in again,
//!   leaving a block, pairing a device;
//! - its menu at the window's top right (`MENU_LAYER`) while nothing waits, which opens what
//!   be-host's organization menu does;
//! - the block stack's dialogs, which its menu opens: unsent edits, invitations, members,
//!   agents, contacts and share links, with the secrets they make answered once
//!   (`host.make-secret`) and held only while they show;
//! - the host's panels its session's editor places (`HostPanel`), which its menu asks for as
//!   be-host's does: its statistics and memory graph, watched only while one shows, and its
//!   menu's rows.
//!
//! - the session's view (core-view's `showing`): its block's editor, which its host opens and
//!   serves, filling the window under everything else, with a notice over it
//!   (`SESSION_NOTICE_LAYER`) while its app loads or after it stopped. Its children come with
//!   4g7b3.
//!
//! Only the guest (`guest`, wasm32 alone) needs more than the constants below, which the tests
//! that run it read natively.

use be_lib_layers::LayerId;

/// The root's own surface, under everything.
pub const ROOT_LAYER: LayerId = LayerId(0);

/// The id of the root's own surface, which `ROOT_LAYER` shows. It and every other surface of
/// the root's but the statistics panel's fill the host's window (`frame-request`'s `window`),
/// at its pixels per point, and follow it as it resizes.
pub const ROOT_SURFACE: u32 = 0;

/// What the root fills its own surface with under everything, sRGB-encoded: the dark theme's
/// background, which its documents' surfaces lie over.
pub const ROOT_COLOR: [f64; 4] = [13.0 / 255.0, 15.0 / 255.0, 20.0 / 255.0, 1.0];

/// The layer showing `TOAST_SURFACE`, over everything else, while a toast shows.
pub const TOAST_LAYER: LayerId = LayerId(3);

/// A surface of the root's own that beui draws its host's notices on, as toasts at its top
/// right: clear but for them.
pub const TOAST_SURFACE: u32 = 4;

/// The layer showing `DIALOG_SURFACE` while the dialog is open, over everything but the lock
/// screen and the toasts, taking every press and the focus.
pub const DIALOG_LAYER: LayerId = LayerId(5);

/// A surface of the root's own that beui draws its dialog on: be-host's own dialog, a card at
/// its width (or a sheet in a narrow window) over beui's scrim, which dismisses it when pressed.
pub const DIALOG_SURFACE: u32 = 6;

/// The title of the question of keeping display settings that just changed, which the root
/// asks in its dialog's place while its host asks it (`host.displays`).
pub const KEEP_DISPLAY_TITLE: &str = "Keep these display settings?";

/// The performance panel's title.
pub const STATISTICS_TITLE: &str = "Performance";

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

/// The title of the list of unsent edits.
pub const UNSENT_TITLE: &str = "Unsent edits";

/// The title of the question the list asks before discarding a block's unsent edits.
pub const DISCARD_UNSENT_TITLE: &str = "Discard edits?";

/// The layer showing `ONBOARDING_SURFACE` while the block stack waits for the person before it
/// opens an organization's blocks (`core-view`'s `waits-for`): over the app and the statistics
/// panel, under the dialog, the lock screen and the toasts, taking every press and, once drawn,
/// the focus.
pub const ONBOARDING_LAYER: LayerId = LayerId(8);

/// A surface of the root's own the size of the window that beui draws what the block stack waits
/// for on: the accounts (be-root-accounts-ui's `AccountsScreen`), the organizations or the
/// sessions.
pub const ONBOARDING_SURFACE: u32 = 9;

/// The title of the organizations while the block stack waits for one to be chosen.
pub const ORGANIZATIONS_TITLE: &str = "Organizations";

/// The title of the dialog that adds an account, before it knows where.
pub const ADD_ACCOUNT_TITLE: &str = "Add account";

/// The title of the dialog while the person signs in in their browser.
pub const SIGN_IN_TITLE: &str = "Sign in with your browser";

/// The title of the question the host asks before signing in again to an account whose sign-in
/// ended (`core-view`'s `reauthenticating`).
pub const REAUTH_TITLE: &str = "Session expired";

/// The title of the question the host asks before discarding unsaved changes (`core-view`'s
/// `discarding`).
pub const DISCARD_TITLE: &str = "Discard unsaved changes?";

/// The title of the organization's invitations.
pub const INVITATIONS_TITLE: &str = "Invitations";

/// The title of the organization's members.
pub const MEMBERS_TITLE: &str = "Members";

/// The title of the question the members ask before removing one.
pub const REMOVE_MEMBER_TITLE: &str = "Remove member?";

/// The title of the organization's agents.
pub const AGENTS_TITLE: &str = "Agents";

/// The title of the person's contacts.
pub const CONTACTS_TITLE: &str = "Contacts";

/// The title of the question the contacts ask before removing one.
pub const REMOVE_CONTACT_TITLE: &str = "Remove contact?";

/// The title of the dialog opening a share link.
pub const SHARE_LINK_TITLE: &str = "Open a share link";

/// The title of the recovery phrase while the person writes it down.
pub const RECOVERY_TITLE: &str = "Save your recovery phrase";

/// The title of what the block stack's failure leaves the person to do.
pub const FAILURE_TITLE: &str = "Something went wrong";

/// The title of the question another device's pairing request asks (`core-view`'s `pairing`).
pub const PAIRING_TITLE: &str = "Add a device";

/// The layer showing `MENU_SURFACE`, the root's menu, at the window's top right while no
/// onboarding shows: over its statistics panel and under its dialog, hit only where its button
/// and its open menu are.
pub const MENU_LAYER: LayerId = LayerId(9);

/// A surface of the root's own the size of the window that beui draws its menu on: its button
/// at the top right, and the menu it opens below it.
pub const MENU_SURFACE: u32 = 10;

/// The square at the window's top right corner its menu's button lies in, in points: the layer
/// is hit there, and where the menu it opens shows.
pub const MENU_BUTTON: f32 = 48.0;

/// The layer showing `SESSION_NOTICE_SURFACE` over the session's editor while it loads, or once
/// it stopped or could not start.
pub const SESSION_NOTICE_LAYER: LayerId = LayerId(10);

/// A surface of the root's own the size of the window that beui draws the session's notice on:
/// its app loading, or why it stopped, as be-host's own UI says it.
pub const SESSION_NOTICE_SURFACE: u32 = 11;

/// The first of the surfaces of the root's own that draw the host panels its apps place
/// (`HostPanel`), one for each place, numbered on from it.
pub const PANEL_SURFACES: u32 = 64;

/// The layers showing the host panels' surfaces: each its surface's number past this.
pub const FIRST_PANEL_LAYER: u64 = 1 << 32;

/// What a host panel the root draws no view of yet says in its place.
pub const PANEL_NOT_DRAWN: &str = "This panel is not drawn under the root yet.";

/// What the session's notice says while its editor's app starts.
pub const LOADING_APP: &str = "Loading app…";

/// What the session's notice says when its host names no app for its block.
pub const NOT_OPENED_TITLE: &str = "This block could not be opened";

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests;

#[cfg(target_arch = "wasm32")]
mod guest;
