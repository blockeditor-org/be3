# The desktop

block-app is also a desktop: its shell is the desktop plugin (`crates/editors/desktop`, plugin
id `be3.desktop`) instead of workspace-ui, on a session of its own, and the host shows other
programs' windows inside it. This guide is what every host shares; each host has its own:

- guides/desktop/linux.md: the Linux host, a Wayland compositor (be-wayland) that also runs
  on the displays themselves under `--session` and installs as a login session.
- guides/desktop/windows.md: the Windows host, which shows Windows' monitors and windows.

`--desktop` (every native platform) starts the desktop shell in a normal window. On Linux
programs open as windows inside it (guides/desktop/linux.md); on Windows it shows Windows'
monitors and windows, launches Start Menu programs and offers power (guides/desktop/windows.md);
on macOS (`src/arguments.rs` parses the flags) it is the shell alone for now: there are no program
windows, notification server, tray, media, power, lock screen or launcher.
The app test `the_desktop_shell_opens_and_closes_cleanly` (crates/app-tests) starts it headless
and closes it. `--session` and `--install-session` are Linux's (guides/desktop/linux.md).

Everything the desktop gets from the host comes through `be3:desktop`, a WIT package
beside the core plugin protocol rather than in it
(`crates/block-plugin-api/wit/deps/desktop`): one interface per part of the session
(`windows`, `input`, `displays`, `power`, `media`, `wifi`, `epaper`, `notifications`,
`screen-lock`, `programs`, `tray`, sharing `types`' `host-image`), each a latest-value watch
(`WindowsWatch`, `PowerWatch`, ...) and, where the desktop asks something of the host, a
function (`power::request(PowerAction)`, `epaper::refresh()`, ...). Another host, a compositor
in another language, can implement it from the WIT alone. A plugin uses it only by naming
`be3:desktop` in its manifest's `extensions` (the desktop plugin, workspace-ui for the program
windows, and settings for displays, input devices and remaps); the host denies a watch or a call
from any other. The desktop plugin shows a control only while the host offers what backs it
(`editor.offered::<W>()`, true once the host answered the watch): the Programs button, the
Super tap and placeholders' Open need `ProgramsWatch`, the bell `NotificationsWatch`, the power
menu `PowerWatch`, the media keys `MediaWatch` (the volume item, its `output`), the Wi-Fi item
`WifiWatch` (its `available`), and the tray its items. block-app publishes each as soon as the
service behind it runs, even empty. block-app publishes the package on Linux, and on Windows its
`windows`, `programs` and `power` (`plugin_host/published.rs`); a host that does not publish it (elsewhere, the web and Android)
answers every watch of the package `unsupported` and drops its calls, which the desktop
sees as nothing offered. The web and Android apps have no `--desktop`: a desktop with none of
these is only a dock of block windows, which their own shells already are.

`./scripts/buck run //crates/block-app:dev -- --desktop` runs the desktop shell headless, to
drive like any other run (guides/running/drive.md).

## Program windows

Where the host opens windows (`opens_windows()`, true only in space), every toplevel is a
window, and so a panel, of its own: workspace-ui shows each top program window as a floating dock
window (`FloatingWindow`) instead of a tab, as the desktop plugin always does, and the dock shows a
window-presented dialog, which is how both shells show a program's dialogs (`AttachedWindow`), as
a `Window` placed `Over` its parent's window, centred on it and `dialog_outset` (the chrome the
styled dock draws around the body) larger than its body. A toplevel's popups are a `Window` of the
`WindowView` (`WindowRole::Popup`, so the toplevel keeps the keyboard and the popup its grab), placed `Over` the window the toplevel is in where the overlay would be, and opened
again whenever the popups' corner moves, as when a submenu grows them leftward or upward. The
compositor still places each toplevel at its rectangle in its own window's coordinates, the
keyboard follows the document's focus as anywhere, and a modal dialog still turns presses on its
parent into raising the dialog. Outside space all of this stays an overlay or a tab, as above.

A window's and its popups' drawings paint too (`Paints` on `WindowDraw`, which reads each surface's
texture back, opaque), so paintings and `drive paint` show what programs drew.

## Notes

`HostWindow::note` is what the host wants said about a window right now, framework-neutral text.
block-shell's `WindowPanel` hands it to its dock tab (`DockTabControl::show_notice`), so the
window's tab or dialog header shows it in place of the title while it is set; nothing is drawn
over the window's content for it. The Windows host sets it for focus mode, hovering and refused
input (guides/desktop/windows.md); the Linux host sets none.

## Shown and hidden entries

`WindowsWatch` lists every entry the host could show: program windows (`kind: window`) and, on a
host that captures them (Windows), whole displays (`kind: display`, `size` the display's, `title`
its name). Each has `shown`, which the host keeps, so a shell started again shows what was shown.
The shells draw only shown entries: block-shell's `Workspace` keeps the whole list
(`Workspace::entries`) and lays out `shown_windows` of it, so a hidden window is no tab, no
floating window and no saved window, and a dialog whose toplevel is hidden is hidden with it.
The desktop plugin's bar has a picker (`desktop.windows`, `app/windows.rs`, shown while the host
offers `WindowsWatch`) listing every toplevel entry, displays first and then windows in the
host's order, each with its icon (the window's own, else a display or window glyph) and title:
choosing one sends `show` (when hidden) then `focus`, and a shown entry's Hide control
(`desktop.windows.entry.ID.hide`) sends `hide`; closing stays `close`. On Linux it is a window
switcher too. block-app's Wayland host opens every window shown and lists no displays; `hide`
only stops the shells drawing it (the client keeps running and connected), and the shown state
lives in `block-app/src/desktop_host/shown.rs` for as long as block-app runs, forgotten when the window closes.
A hidden window shown again opens at its default place, not where it was.

## Displays

The host shows each display in a window of its own (under `--session`, guides/desktop/linux.md,
Displays). The desktop is one space in points: the displays side by side along their bottom edges. Each
display's window shows its own region of the shell, macOS-like: the first display the shell's
frame (`ui::Root`'s `MainSurface`), each further one a `display(n)` region of the same instance
(`ui::SecondaryDisplay` mounts `compositor::ShellDisplay`, n being the display's place in the
list from 1), each at its display's size and scale. The shell's windows are the host's own,
placed in the desktop's points: the shell's regions say the host shows the windows the plugin
opens from them (`windows` in the screen request, set for the shell's frame and display regions
and for the windows themselves; guides/adding_a_plugin_editor.md), so every `Window` the shell
opens, each of the desktop plugin's dock windows, becomes a `window(w)` region that block-app keeps
in `compositor/desktop.rs`: a list of `Floated` windows, each its rect in desktop points, in
stacking order (a press raises one). The shell's region on each display draws, between its own
surface and its floating layers (menus, popovers, toasts), the windows that display holds most
of (`desktop::shown_on`, `DesktopWindows` in `compositor/region.rs`), each a `PluginRegion` of
its window region placed at its rect less the display's corner, whose children are the program
windows and block editors the shell placed in it. A window opens where the plugin asked: at a
point of the region it opened from (`Over`, measured from that region's corner on the desktop),
at the pointer, or at the `placement` it kept (`desktop X Y`); the host answers with that
placement on the window's screen request, which the dock keeps. `BeginMove` and `BeginResize`
hand the press to the host: the window follows the pointer from where it was pressed (the
press's point and the window's rect are noted on the press, `desktop::follow`), the plugin sees
no moves meanwhile, and the release ends it and gives the new placement. While one moves it is
drawn on every display it overlaps and on the one the move began on, which holds the press;
once let go it is drawn only on the display holding most of it. Each region is laid out at the
document's scale and rendered at the raster of the display that shows most of it (each mount's
`EmbedPlacement::raster`, `RegionPlacement::raster` of the offer whose clip is largest), so a window moved onto a display at another scale is rendered at that scale once
it is mostly there; while it straddles two, the display that holds less of it shows the same
texture scaled. Program windows and block editors are children of the windows they are tabs
of, so they stack and take the focus with them. The pointer crosses between display windows
with a press held, by beui's cross-window rules (the platform reports each window's desktop
origin, `WindowReport::Moved`), and between the shell's regions by the protocol's `over`: a
press held in one of the shell's regions names the region the pointer is over, so a tab dragged
out of one window and over another, on any display, lands on that window's bar.

`--desktop` in a window, or headless, is the same with one display, the window: the shell's frame
fills it, and its windows are the host's own, drawn over it in that window's points. The plugin
tests do the same through block-ui-test's `BeuiTest::show_on_displays`, which shows the regions
and hosts the windows as block-app does, takes input in desktop points (`desk_press`,
`desk_move`, `desk_release`, `desk_drag`) and paints a display with the windows on it
(`desk_snapshot`); without it a plugin draws its windows inside its frame.

The shell learns the displays from `DisplaysWatch`: each `HostDisplay` carries its `rect` in the
desktop's points and its `scale`. The desktop plugin draws, in every region, a bar along its foot and
a level display (`desktop.osd.{n}`, n the region's display), and in the frame alone its dock,
launcher, pickers and notification toasts (above that display's bar); `BeuiApp::Display`
draws the further displays' regions from what the frame's region shares (`app.rs`, `Desktop`).
The dock fills the frame above the bar, and its windows open over it.

## A shell that stops

When the shell's plugin stops (a panic, a trap, a hang or its session failing; any stop
`Runtime::crashed` reports), block-app's `watch_shell` closes it and every editor under it,
drops its runtime and shows the host's own session and profile picker (`Screen::Sessions`)
with the error toast and its Copy report. Choosing a session or profile, or keeping the current
one, opens a fresh shell. The windows stay. On Linux the compositor keeps every client connected,
and `Compositor::set_owned(false)`, which block-app sets whenever no shell is on screen, takes
the keyboard, the pointer, held buttons and popups from them, drops any input meant for them
and pings every client each `PING_TIMEOUT`, so one that froze meanwhile is listed as not
responding. The lock screen is the host's and works throughout. The new shell sees the
windows in `WindowsWatch` and shows them as it shows any window; the desktop plugin's saved layout
names window tabs by `HostWindowId`, which the host keeps, so a restored layout puts
each window back where it was, fullscreen and dialogs included. To try it, open the Plugins
panel from the bar's menu and press Crash beside `be3.desktop`.
