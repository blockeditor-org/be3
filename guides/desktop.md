# The desktop

be-host is also a desktop: under `--desktop` its shell is the desktop app (`crates/be-app-desktop`,
app id `be3.desktop`) instead of workspace-ui, and the host shows other programs' windows inside
it. This guide is what every host shares. The rest:

- guides/desktop/shell.md: the desktop app itself: its bars, Quick settings, the Apps panel, the
  window chips and the window keys.
- guides/desktop/linux.md: the Linux host, a Wayland compositor that also runs on the displays as a
  login session (`BEUI_PLATFORM=drm`), and its parts under guides/desktop/.
- guides/desktop/windows.md: the Windows host, which shows Windows' own monitors and windows.

`--desktop` is parsed in be-host's `src/arguments.rs` for every native platform. On macOS it is the
shell alone, with no program windows, tray, media, power or lock screen. The web and Android have
no `--desktop`.

Only `--desktop` hosts program windows: `HostCore::desktop`, set by `run_as_desktop`, makes be-host
start the platform's `DesktopHost` (Linux's compositor and Xwayland, Windows' window watcher), and
only then are the package's `windows` and `programs` offered. Any other run answers those two
`unsupported`, so it lists no program windows, no programs to launch and no program as an editor of
a block. A desktop refuses to start on a data directory another be-host already runs on
(guides/running/native.md).

To run it headless: `./scripts/buck run //crates/be-host:dev -- --desktop`, then drive it
(guides/running/drive.md). The host test `the_desktop_shell_opens_and_closes_cleanly`
(crates/be-host-tests) does the same. The desktop app's own tests run the shell on be-sdk-test's
desk (Displays, below).

## The `be3:desktop` package

Everything the desktop gets from the host comes through `be3:desktop`, a WIT package beside the core
app protocol (`crates/be-sdk-api/wit/deps/desktop`). It has one interface per part of the session
(`windows`, `input`, `displays`, `power`, `media`, `wifi`, `epaper`, `notifications`,
`screen-lock`, `programs`, `tray`), each a latest-value watch (`WindowsWatch`, `PowerWatch`, ...)
plus functions where the desktop asks something of the host (`power::request`, `epaper::refresh`).
A compositor in another language could implement it from the WIT alone.

- An app uses it only by naming `be3:desktop` in its manifest's `extensions`: the desktop app,
  workspace-ui (for program windows) and settings (displays, input devices, remaps). The host
  denies every other app.
- What is published: `be-host-runtime/src/published.rs`. On Linux, the whole package (its
  `windows` and `programs` only under `--desktop`); on Windows, under `--desktop` only, `windows`,
  `programs` and `power`. Elsewhere every watch answers `unsupported` and calls are dropped.
- Each watch is published as soon as the service behind it runs, even empty.
- The desktop app shows a control only while the host offers what backs it
  (`editor.offered::<W>()`), so an unsupported interface simply hides its controls. Settings does
  the same at a finer grain: `InputReachWatch` says where keymap, key repeat and middle-click paste
  take effect, `InputDevicesWatch` gates pointer settings and remaps, `DesktopBarWatch` the bar's
  buttons.

## The host side

be-host reaches the platform through `be_host_desktop::desktop_host::Host`, the platform's
`DesktopHost` (crates/be-host-desktop): a trait of associated functions (start, list and act on
windows, launch) whose defaults do nothing, so a platform with no desktop writes none. Its `Source`
is the `ProgramSource` that lists installed programs and runs one; `programs/installed.rs` is the
one `Programs<S>` over it, which scans on a thread, holds launches until the list is in and
publishes `ProgramsWatch`.

Work off the UI thread hands its results back through `be_host_base::host::mailbox`: a worker
`Post::send`s a value, which wakes the event loop, and the handler runs on the UI thread at the
start of a frame, so nothing polls. Dropping the `Mailbox` drops whatever is still on its way. A
handler that needs `&mut` state its owner holds is made on a `host::Desk<C>` and runs when the owner
calls `desk.deliver(&mut c)` (`HostCore`'s requests, logind, `Programs`, `ScreenLock`).

The host keeps what it knows of each window in be-host-desktop's `KnownWindow` and publishes only
its `listed()` part. `be_host_desktop::naming::window_name` names a window everywhere it shows: the
program's name, then " — " and the window's title when it says more. The name comes first from
the program the desktop launched the window from (`origin.program`), else the desktop entry (Linux)
or Start menu program (Windows) matching the window's program id.

## Program windows

`WindowsWatch` lists every entry the host could show: program windows (`kind: window`) and, on
Windows, whole displays (`kind: display`). Each entry (`host-window`) holds only what the shell
needs to place it: id, kind, parent, modal, program id, origin, the size the program picked and
whether it asked to be maximized. Only the shell instance gets
this watch; a session opened inside the desktop is a block window and sees no program windows.
Everything else about a window is its child's `child-status`, and the shell acts on it as on any
child.

**A program window is a tab.** be-sdk-shell's `Workspace` has one tab model whose content is a
block or a program (`program.rs`: `Program { window, record }`). Program tabs take keys from the
same counter as block tabs and are saved with the dock layout in the `layout` view state
(`saved::SavedProgram`). The desktop app floats each in a dock window of its own (`ProgramWindow`);
workspace-ui puts them in its editors pane, or floating where the host opens windows (in space).
`program::place` binds windows to tabs.

**Child status to the tab** (`program::WindowBody`, `Workspace::hear`): `title` and `icon` onto the
tab, chip and dialog header; `group` onto `DockTab::group`; a count up of `raised` shows the tab
and gives it the keyboard; `fullscreen` toggles the tab's full screen, whose state goes back as
`child-placement.fullscreen`; `rung` flashes the chip; `notice` replaces the tab's title while set
(the Windows host uses it to say how to get back; Linux sets none); `menu-asked` opens the tab menu;
a count up of `minimized` hides the tab's dock window; `intrinsic` is the size the program picked,
which only a dialog's card follows. A new window's dock window opens at the listing's `size` (or a
default size when there is none), snapped to fill its area when the listing says `maximized`, so
the program is never told another size first; a window
filling a placeholder takes the placeholder's place and size (`program::cascaded`).
`close-child` asks the program to close; the tab stays until the window leaves the list.

**Shown and hidden.** The host keeps no shown state: whether an entry shows is the dock's. A
hidden dock window (`DockHostLayout::hide_window`) places no body for its tabs, so `UnshownWindows`
(placed by both shells beside `WorkspaceDialogs`) keeps every listed window the dock does not place
(`DockHostLayout::is_shown`) placed with `ChildMode::Hidden`. It takes no room and draws nothing,
the host suspends it (Linux) or stops capturing it (Windows), but its status keeps arriving, so a
hidden window's raise can show it again. The first status heard is taken as it stands, so a
restarted shell does not replay old raises or bells.

**Dialogs** (a toplevel with a parent: an xdg parent or xdg-dialog, an X11 transient, a Windows
owned window) are not tabs: be-sdk-shell's `WindowDialog` declares each as a `DockDialog` of its
parent's tab, drawn as the dock's anchored card. A parent missing from the list does not count
(`listed_windows`), and a parent can appear after the window shows (GTK 3 sets it after the first
commit), so a window moves between tab and card. A modal dialog dims its parent and takes the
keyboard when the parent is pressed.

**Popups** of a toplevel are a `Window` of role `WindowRole::Popup` placed `Over` the window the
toplevel is in, opened again whenever their corner moves.

**Placeholders.** A program tab with no window draws "X is not running" with Open and Dismiss.
Open launches with the tab's key as the launch id (`ProgramAction::Launch { launch: Some(key) }`),
so the window fills that tab. A window whose program cannot be started again leaves no
placeholder. When `PowerAvailability` says the session is ending, closing windows leave
placeholders (`Workspace::keep_through_ending`), so nothing open when a restart begins is lost.
Window ids never repeat across runs, so a saved record is only adopted by its own window. Programs'
arguments are not saved (decisions/kept_windows_store_no_program_arguments.md).

**Drawing.** be-host-desktop's `PlacedWindow` draws a placed window child: the platform's
`WindowSurface` inside `WindowBody` (`window_body.rs`), which fades where the program's buffer
overhangs the rect, fills with the theme's background what the buffer does not cover yet (a window
being resized before the program draws the new size), says "Too small for" below the program's least size, and draws the "not
responding" Wait and Force close. Window drawings paint (`Paints` on `WindowDraw`), so paintings and
`drive paint` show what programs drew. be-host's `compositor/editors.rs` turns `close-child`,
fullscreen and the keyboard into requests to the program.

## Displays

The desktop is one space in points: the displays as arranged in Settings > Display, or side by
side along their bottom edges. `DisplaysWatch` gives each `HostDisplay` its `rect` and `scale`.

- Each display gets a window with its own view of the shell: display 0 the main view (`ui::Root`),
  each further one a `HostView::Display(n)` of the same instance (`ui::SecondaryDisplay`). The
  desktop app draws in each a dock with that display's bars and its level display; the frame alone
  draws windows, pickers and toasts (`root.rs`, `DesktopRoot`).
- The shell's views ask the host to show the windows they open (`windows` in the screen request),
  so every dock window becomes a host window view (`HostView::Window`) placed in desktop points.
  be-host keeps them in `compositor/desktop.rs`.
- The window manager without signals is the `be-sdk-window-stack` crate: window requests (`ask`),
  stacking (`raise`, `settle`), moves and resizes, work areas (`keep_in_work_areas`, `fitted`,
  `home`) and placement. be-host wraps it in signals, and be-sdk-test's desk runs it directly, so
  app tests drive the same window manager.
- Every screen request carries the view's `origin` in desktop points; the dock keeps each window's
  rect from it (`WindowReport::Moved`), and asks to move a window with the `place` request.
  `BeginMove`/`BeginResize` hand the press to the host.
- A display's work area is the display less its bars' bands (`DockHostLayout::area`). The shell
  reports it per view (`frame-report.work-area`), and windows are kept wholly inside it on edges a
  band insets, except a `WindowLevel::Fullscreen` window (the dock's fullscreen), which covers the
  whole display; a window on no display (unplugged) moves to the nearest.
- A window is drawn on the display holding most of it, at that display's raster scale; while it is
  moved, on every display it overlaps.
- The shell's view draws its surface, every child it places (the tabs docked in its own dock)
  and its marks over them (`frame-report.marks`: a splitter's line and reach) under the
  windows, and only its overlays (menus, scrims) above them, for drawing and input (`AppView`).
- A raised dock bar (`bar_level` `Raised`) is a floating layer above windows; a bar popped into its
  own window opens at `WindowLevel::Above`. A fullscreen window drops bars to `Normal`.
- The pointer crosses display windows with a press held (beui's cross-window rules) and between the
  shell's views by the protocol's `over`, so a tab dragged from one window lands on another's bar
  on any display.

A windowed or headless `--desktop` is one display, be-host's window. In be-host's own window with no
desktop (`OverlaidWindows`), windows are kept reachable inside it (`desktop::reachable`,
`carried`, `clamped`).

App tests use be-sdk-test's `BeuiTest::show_on_displays`, which hosts the views and windows as
be-host does: input in desktop points (`desk_press`, `desk_drag`, `desk_moves`, and `click`,
`hover`, `drag` by test id), and `desk_snapshot` paints a display with its windows and layers.
Without it an app draws its windows inside its frame.

## A shell that stops

When the shell app stops (panic, trap, hang, session failure, or Kill in Running Apps), be-host's
`watch_shell` closes it and its editors, and shows the host's session and profile picker
(`Screen::Sessions`) with a Copy report toast. Every stop goes through `Runtime::stopped`, which
reports it once to `take_stopped`. Choosing a session opens a fresh shell; the program windows
stay. On Linux `Compositor::set_owned(false)`, set whenever no shell is on screen, takes input from
the clients and pings them every `PING_TIMEOUT`, so a client that froze meanwhile is shown as not
responding. The new shell restores each window into the tab that last held its `HostWindowId`. To
try it: Apps panel > Menu > Running Apps, then Crash or Kill beside `be3.desktop`.

## A layout that cannot be read

When the `layout` view state does not decode, the desktop starts nothing from it and saves nothing
over it: be-sdk-shell's notice (`workspace.layout`; guides/blocks/workspaces.md, View state)
offers Reset layout, Exit and Copy report. Running programs show behind it as windows but are not
saved until a reset. Exit is Log out in a login session. The same holds for further displays' bar
layouts (guides/desktop/shell.md).
