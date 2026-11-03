# Program windows as children, and the dock as the window manager

One PR, run by a coordinator with subagents (guides/parallel_agents.md). Each step below names
what it changes and what it removes. Later steps build on earlier ones; the coordinator lands them
on the integration branch in order.

## Why

Program windows (Wayland toplevels, captured Windows windows) reach the shells today as the
`be3:desktop` `windows` interface: a `WindowsWatch` list of rich `host-window` records and a
`windows::request(window-action)` call. Each shell reads those records and runs a second tab model
beside its block tabs: `WindowTab` and its `WINDOW_TABS + id` tab ids, size negotiation
(`followed_size`, `within_limits`, min/max), focus/fullscreen/hide/close sync, kept windows and
placeholders (`kept.rs`, `saved.rs`, the desktop's `placeholders.rs`), and a window manager
(`be-sdk-shell/src/manager.rs`). About 2,100 lines of shell code exist only because windows take a
different path from every other child.

A placed `SubregionContent::Window(id)` child is already drawn by the host (`Host::WindowSurface`).
This plan makes that child an ordinary child: what the shell needs to know about it arrives in its
`child-status`, what the shell asks of it goes through the calls a placer already has for its
children, and the shell's view tree places it exactly as it places a block tab. The window manager
(snap, maximize, hide, show desktop, move between displays, cycle windows) moves into beui-dock as
actions any shell can bind.

## Decisions (from the user)

- A program window is a child placed in a dock tab, like a block. Its tab is placed by the shell's
  tree: workspace-ui puts it in its editors pane, the desktop in a floating dock window of its own.
  The person can dock it anywhere after.
- No size negotiation and no min/max. The dock resizes a window frame whenever it likes; the host
  tells the program the new size and draws the latest buffer anchored top-left, clipped to the
  frame. Where the buffer is clipped, the host shows beui's scroll fade along that edge; while the
  frame is smaller than the program's least size, it shows a short "too small" notice. The dock
  enforces no size rules. On Windows the host resizes the real window (`resize_window`) the same
  way; a window that never takes the size is drawn top-left with empty room bottom-right.
- A placeholder ("Firefox is not running") is a tab whose content is a program that is not
  running, with Open (relaunch) and Dismiss. The desktop keeps its Restore chip, which opens or
  dismisses every waiting placeholder at once. A program with no way to relaunch it gets no
  placeholder.
- The window manager lives in beui-dock as actions with no key bindings. The desktop binds keys to
  the ones it wants. The actions work on any dock window, block windows included, and in XR too:
  XR only places the dock's floating windows in space.

## Protocol

### Core app protocol (`crates/be-sdk-api/wit`)

`child-status` (shell.wit) gains what any child may report about itself. A block editor reports
none of these yet; the host fills them for a `window` child:

- `title: option<string>`: the child's own name for its tab, chip and header. For a window, the
  host's name for it (today's `be_sdk_shell::window_name`: program name, then " — " and the title
  when it says more; the naming code moves into the host, be-host-desktop).
- `icon: option<picture>`: a new core record `picture { width: u32, height: u32, rgba: list<u8> }`
  in types.wit (rows of rgba pixels). For a window, the program's icon.
- `group: option<string>`: which children are one app's. For a window, its program id. The dock
  cycles a group's windows (`next_of_group`).
- `raised: u64`: counts up each time the child asks to be brought forward (a program activating
  its window, a press on it). The placer shows the child's tab.
- `rung: u64`: counts up each time the child asks for attention (a program's bell). The placer
  flashes its tab and chip.
- `fullscreen: bool`: whether the child asks to be shown full screen (a program's own
  set_fullscreen). The placer shows the tab full screen while it is set.
- `notice: option<string>`: what the host wants said about the child right now (today's
  `host-window.note`); the placer shows it with the tab's title.
- `menu-asked: option<menu-ask>`, `record menu-ask { asked: u64, at: point }`: the child's last ask
  for its tab's menu at `at`, in the child's points (a program's show_window_menu). The placer opens
  the tab's menu there.

`intrinsic` (already there) carries a window's own size as its program last drew it, before the
shell first sizes it. A new floating dock window opens at its child's intrinsic size.

Not responding needs nothing in the protocol: the host draws the window's body, so it draws the
"not responding" banner, with Wait and Force close, over it itself.

`child-placement` (types.wit) gains `fullscreen: bool`: the placer shows the child full screen.
The host tells a program window it is full screen while it is set.

shell.wit gains `close-child: async func(instance, child)`: the person asked to close a child the
editor placed. For a window the host asks its program to close it (or force-closes one that does
not respond, as `window-action.close` does today); the window leaves the list when it closes. For
any other child it does nothing yet.

Keyboard focus uses what children already have: `focus-child` and the keyboard the host gives the
view that shows the child. The host routes them to the window (today `WindowView` ignores them and
takes focus only from a press or its own raise counter).

Hidden needs nothing: a window whose tab is not shown is not placed, and the host tells a program
whose window is placed nowhere that it is suspended (xdg `Suspended`), as `hide` does today.

### `be3:desktop` `windows` (`crates/be-sdk-api/wit/deps/desktop/desktop.wit`)

`host-window` shrinks to what the shell needs to place a window and match it to a placeholder:

```
record host-window {
    id: host-window-id,
    kind: window-kind,          // window or display
    parent: option<host-window-id>,
    modal: bool,
    program-id: string,
    origin: window-origin,      // program, command, launch: for placeholders and relaunch
}
```

`windows::request` and `window-action` go. `windows-watch` and `super-kept-watch` stay. Everything
else (title, name, icon, size, min/max, fullscreen, responding, focused, raised, rung, note, menu,
shown) moves into the window child's status or goes. The host keeps no `shown` state.

Steps 1-2 add the new pieces beside the old ones; step 4 removes the old ones.

## The shells

be-sdk-shell's `Workspace` has one tab model. A tab's content is a block (as today) or a program:
`Program { window: Option<HostWindowId>, program: ProgramRecord }`, where the record (program id,
desktop entry, command, name, title) is what the tab saves so it can be a placeholder after a
restart. A program tab with no window is a placeholder; its body (drawn by be-sdk-shell) says the
program is not running and offers Open and Dismiss. Open launches the program with the tab's key as
the launch id (`ProgramAction::Launch { program, launch: Some(key) }` or `Exec`).

A window in the `WindowsWatch` list with no parent goes, in order, to the tab whose key is its
`origin.launch`, to the oldest waiting placeholder of its program id, or to a new tab. A window
with a parent is a dock dialog of its parent's tab, as `WindowDialog` does today. The tab's body
places `SubregionContent::Window(id)` and maps the child's status onto the dock tab: `title`/`icon`
onto the tab, `raised` to showing the tab and `focus-child`, `fullscreen` to the tab's full screen
(and the placement's `fullscreen` back), `rung` to its attention, `notice` to its notice,
`menu-asked` to opening its menu; closing the tab is `close-child`. A window that leaves the list
closes its tab, unless the shell is keeping it as a placeholder (the session is ending,
`keep_through_ending`).

Removed from be-sdk-shell: `WindowTab`, `FloatingWindow`'s window logic, `kept.rs`, `saved.rs`,
`manager.rs`, and from `window.rs` everything but the placed body (`within_limits`,
`followed_size`, `cascaded`, `snapped_short`, naming). Removed from the desktop: `placeholders.rs`
(its Restore chip stays, over the placeholder tabs), and `windows.rs` keeps only the chips, built
from the dock's state and the tabs' statuses.

The saved layout format of the shells' view blocks is not frozen (guides/frozen_formats.md): kept
windows saved by an older build may be dropped.

## beui-dock as the window manager

`DockingLayout` gains, for its floating windows:

- areas: the rects of the displays the dock spans (`set_areas`), so a window belongs to the area
  it overlaps most, snaps within it, and moves to the next or previous one keeping its place;
- groups: a `group` on `DockTab`, the key the cycling actions use;
- hidden windows: a window can be hidden and shown again (its tabs are then not placed); show
  desktop hides every window and a second press brings them back.

and a set of `Action`s with ids and labels but no keys, each with its enabled state: close, hide,
snap left, snap right, maximize, restore, full screen, show desktop, next area, previous area,
next of group, and the switcher (today's `be-app-desktop/src/root/switcher.rs`, cycling windows in
most-recently-shown order). A shell passes in whatever disables them (the desktop's lock screen).
The desktop binds chords to them (`window_keys.rs` keeps only its chord table); workspace-ui
binds none for now. beui-demo shows a dock with floating windows and the actions in a menu.

## Steps

1. Protocol additions and plumbing: the new `child-status` fields, `picture`, `menu-ask`,
   `child-placement.fullscreen`, `close-child`, `host-window.origin` unchanged; be-sdk's
   `ChildState` fields, `Editor::close_child`, `Subregion`'s `fullscreen`; be-host-runtime's
   `HostChildStatus` and the wit mapping, defaulting every new field. be-sdk-test's fake host lets
   a test set a window child's status. Nothing uses them yet.
2. In parallel:
   - 2a. beui-dock window manager (above), and beui-demo.
   - 2b. Host: be-host fills a `Kind::Window` child's status from a per-window accessor on
     `DesktopHost` (title, icon, group, raised, rung, fullscreen, notice, menu-asked, intrinsic),
     honours `child-placement.fullscreen` and `close-child`, routes the given keyboard and
     `focus-child` to the window, suspends a window placed nowhere, configures every placed window
     to its rect (no own-size exception) and draws it top-left, clipped, with fade edges, the "too
     small" notice and the not-responding banner. Linux and Windows.
3. Shells: one tab model with program tabs and placeholders, the beui-dock actions, chips from
   the dock; be-app-desktop, be-app-workspace and be-sdk-shell move to the new child status.
4. Removal: `windows::request`, `window-action` and the moved `host-window` fields, the host's
   `SHOWN` and `Model.shown`, and whatever reads them.
5. be-sdk-shell keeps only what both shells use: the picker, rename, revisions and phone code
   move to be-app-workspace, and `Workspace::new` loses `with_files` and `keeps_windows`.
6. Guides: guides/desktop.md, guides/desktop/linux.md, guides/desktop/windows.md,
   crates/be-sdk-api/PROTOCOL.md, guides/beui.md (the dock), each updated by the step that changes
   what it describes; this plan is deleted at the end.
