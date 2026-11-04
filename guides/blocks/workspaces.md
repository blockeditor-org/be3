# Organizations, workspaces, sessions and view state

Part of the block stack (guides/the_new_block_stack.md). How the host lays an
organization out in blocks. Block, workspace and organization settings,
resolution and settings access are guides/settings.md; this file covers
device profiles, the settings the host applies itself, sessions and view
state.

## Organizations and workspaces

The account switcher picks an account, the Organizations screen one of its
organizations (opening the only one by itself), and everything below happens
inside that one. A workspace is any root block (`BlockParent::Root`) that is
not a system block (`be_sdk_beui::sections::is_workspace`); its name is its
`NAME` and its members are its grants. New ones are of
`be_block::WORKSPACE_TYPE` (a folder-shaped `workspace_ui_types::Workspace`).
Only workspaces and an account's own `Settings` are put at the root, though
the server accepts any block there, so an old one left at the root shows as a
workspace. Opening an organization ensures the account's "My files"
(`RootSettings::ensure_my_files`, the `MY_FILES` key).

The files pane (`crates/be-app-file-tree`) shows a section per visible
workspace (My files first, never another account's), Shared with me
(`BlockQuery::Shared`), Recently Deleted, and System (the account's own
`SYSTEM_TYPES` roots, for debugging). Pressing a folder, workspace or section
zooms into it rather than opening it. Every picker places a new block in a
workspace (My files when none is named); moving between workspaces is the
host's `move_block`. The folder editor is `crates/be-app-workspace-index`.

## The account's Settings block and device profiles

Each account keeps what is its own in an organization under one top-level
`Settings` block it authored, found by author (an administrator sees every
account's). When two devices each made one before seeing the other's, both are
read as one (`Settings::merged`, lowest id winning a clash) and written through
the lowest. The settings editor shows someone else's `Settings` or profile
with a notice and changes nothing.

A settings profile is a `SettingsProfile` child of `Settings` (listed in its
`profiles`) with `kinds` of its own and a `class` (`DeviceClass`: Desktop,
Phone, Tablet, E-paper tablet). Each device uses one (`device_profiles`, keyed
by client id); be-host makes one named after the class the first time a device
opens the organization. `root_settings::device_class` picks E-paper tablet when
the session found an e-paper panel, Phone on Android, Desktop otherwise; Tablet
is only picked by hand.

A class's defaults are listed per settings type in one place
(`DeviceClass::display_defaults`, `display_edits` when the class changes).
`ensure_profile` writes them into the profile's own blocks, and when there are
any the host opens the profile's page (`SettingsProfileView`,
`be-app-settings` `root/setup.rs`), which shows "Set up this device" while the
profile is this device's and still keeps defaults that differ from every
device's value. Nothing is stored to say a profile was set up.

## Writing a layered setting

A change syncs by default: it goes to the every-device block. A setting is kept
on this device exactly when this device's profile block sets it; nothing else
records that. A screen known only by its connector (no EDID serial) is always
this device's alone. Each field is described once as a `be_block::Setting`
(`DisplaySettings::theme_setting()`, `mode_setting(id)`), and
`Setting::layer` is the one rule for which layer a write goes to; every write
goes through it (`be_sdk_beui::LayeredSettings` in apps, `display::keep` and
`revert` in be-host). `LayeredSettingsRow` gives every row Keep on this device
and Reset to default (`<id>.keep`, `<id>.reset`); `ChangesToast` names what
now applies on every device with Keep on this device only and Undo.

The settings app's pages use beui's `SettingsPage`, `SettingsSection`,
`SettingsRow` and `SettingsLink`, and show a section only where the host
offers the matching host value (`Displays`, `Power`, `InputDevices`; only a
login session publishes `Power` and `InputDevices`). Devices and profiles
(`root/devices.rs`) tables what each profile keeps.

## Settings the host applies itself

The host holds some settings blocks itself (`be_host_stack::hold`). A kind it
applies implements `local_settings::LocalSettings` and is kept by a
`SettingsSync`, whose `apply` answers whether the settings may be saved now.

- `sync_local_settings` hands the input settings to the login session's seat
  (`beui_platform_drm::InputControl`) and nested Wayland clients, and the
  display settings to the host and the seat.
- `DisplaySettings` holds the theme, motion and scale (`sync_ui_settings`: the
  host's layer of beui's preferences, so an unset one follows the system,
  guides/beui.md; scale multiplies into the screen's pixels per point), a
  `ScreenSettings` per screen (mode and rotation, keyed by EDID make, model and
  serial or by connector; applied live through
  `beui_platform_drm::DisplayControl`, guides/desktop/linux.md), bar buttons,
  and screen-off and lock times.
- Every setting is an `Option`; unset means the default libinput or the monitor
  reports, read by editors as the `InputDevices` and `Displays` host values.
  Seat, settings and host values share types from `be-lib-ui-preferences`, so
  nothing converts between them.
- The seat runs before anyone signs in, so the host keeps the last applied
  settings of each kind in `host_state` and applies them at startup; the block
  stays the source of truth.

Display settings are not saved until someone keeps them. A mode change that
moves what a monitor shows (a rotation saves at once), or a monitor at a mode
this device never kept, opens the host's "Keep these display settings?" prompt
(be-host `ui/keep_changes.rs`, from `display::guard`); it is the host's because
it has to work before any shell app runs. Each change is a round
(`Guard::prompt`) with its own countdown; Revert, Escape or 15 s of frame clock
put back the earlier modes. The modes each monitor showed once answered are
device memory (`host_state`'s `tried_display_modes`), not a setting. Writes go
by the layer rule as edits on each layer's own content
(`SettingsSync::layers`), so a screen's register never picks up a rotation it
only inherits.

## Sessions

A session is what a person has open, separate from the settings, so a new one
keeps them. It is a view block of a shell editor: workspace-ui
(`WORKSPACE_EDITOR`) or the desktop app (`DESKTOP_EDITOR`, for
`be-host --desktop`). `Settings::sessions` lists them, and
`Settings::session(shell, client)` is the one this client uses for a shell, so
the desktop and the workspace shell keep their own. The host opens the shell
on it (creating one named after the device when missing) and reopens the
shell when it changes; its sessions screen picks profile and session.

A shell keeps its dock layout and recents in the session's view state. Every
tab, and the file tree, is a view block child of the session that the layout
references; closing a tab detaches its view. A tab on a block that is itself a
view block uses it as its view.

## View state

A view block is any block whose defined metadata names the editor that draws
it in `VIEW_EDITOR` (the editor's manifest `id`, not a block type), set with
`BlockMetadata::view(editor, name)`. That key is all anyone reads to know a
block is a view and which editor opens it (the host skips `DELETED_FROM` for
one, and a block's places leave views out).

The content is `be_block::EditorView { content, state }` (`VIEW_TYPE`, served
by workspace-ui's type module): the block shown, and a `LatestMap` of opaque
`ViewState { bytes, refs }` the editor owns. `references()` is `content` plus
every entry's refs, and a deleted or replaced child is blanked or repointed in
place, so bytes may index into `refs`. An editor learns its view block from
`Open { view_block }` (`ChildTarget::viewed_by(view)`) and uses
`editor.view_state(key)`/`set_view_state`, which skip editability since view
state is the viewer's own. An editor on no content (the file tree,
workspace-ui) is opened on its own view block, names no `edits` in its
manifest, and so is never offered by Open with. `be_block::profile` holds the
shells' and file tree's ids.

Both shells are built from `crates/be-sdk-shell` (`Workspace`: tabs, views,
saved layout, block menus, dialogs, host panels). The layout is the session's
`layout` view state (`saved::SavedLayout`, postcard). One that does not decode
is kept, not replaced: `Workspace` builds and saves nothing from it
(`unreadable_layout()`), and `WorkspaceDialogs` shows `workspace.layout` with
Copy report, Exit (`Workspace::exit`, `close-editor` on the shell's own block)
and Reset layout, so a build that can read the bytes still finds them.
