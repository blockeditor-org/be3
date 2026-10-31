# The desktop

block-app is also a desktop: its shell is the desktop plugin (`crates/editors/desktop`, plugin
id `be3.desktop`) instead of workspace-ui, on a session of its own, and the host shows other
programs' windows inside it. This guide is what every host shares; each host has its own:

- guides/desktop/linux.md: the Linux host, a Wayland compositor (be-wayland) that also runs
  on the displays themselves as a login session (`BEUI_PLATFORM=drm`) and installs as one.
- guides/desktop/windows.md: the Windows host, which shows Windows' monitors and windows.

`--desktop` (every native platform) starts the desktop shell in a normal window. On Linux
programs open as windows inside it (guides/desktop/linux.md); on Windows it shows Windows'
monitors and windows, launches Start Menu programs and offers power (guides/desktop/windows.md);
on macOS (`src/arguments.rs` parses the flags) it is the shell alone for now: there are no program
windows, notification server, tray, media, power, lock screen or programs to launch.
The app test `the_desktop_shell_opens_and_closes_cleanly` (crates/app-tests) starts it headless
and closes it. `BEUI_PLATFORM=drm` and `--install-login-session` are Linux's (guides/desktop/linux.md).

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
(`editor.offered::<W>()`, true once the host answered the watch): the Apps panel's program
search and grid, the Super tap and placeholders' Open need `ProgramsWatch`, the clock's notification count and
history `NotificationsWatch`, the power row `PowerWatch`, the media keys `MediaWatch` (the
volume icon and controls, its `output`), the Wi-Fi icon and tile `WifiWatch` (its `available`),
and the tray its items.

## The bars

The shell is three dock-level `DockBar`s of the desktop's own dock (`desktop.dock`), declared in
`DesktopBody` (`app/bar.rs`), each a `DockBarSurface` with a `DockBarGrip` first:

- Apps (`desktop.bar.apps`, `AppsBar`): the Apps button (The Apps panel, below).
- Windows (`desktop.bar.windows`, `WindowsBar`): the window chips (Shown and hidden entries).
- Status (`desktop.bar.status`, `StatusBar`): the tray, the device tools, the status button and
  the clock.

Every display has its own three (`bar::Bars::of(display)`: block-shell's `SHELL_BARS + 3 * display`
and the two after it), which it places and hides by itself. By default all three sit beside the
bottom edge in one band (`dock.bar.band.bottom`): Apps at its start, Windows centred, Status at its
end. A grip moves its bar like any dock bar: beside any edge at its start, centre or end, floated over
one, or popped out into a window; the dock saves where each is and whether it is hidden, in the
session's layout. Every part lays itself out along its bar (`bar::bar_direction`, from
`use_dock_bar().direction()`): down a left or right edge the chips show only their icons, the clock
stacks its hours over its minutes and the status icons stack too. A button's panel (`BarPopup`)
opens away from the bar's edge, toward the windows: below or above a bar along the top or bottom
(`PopoverPlacement::Below`, which turns up when there is no room below), beside one down a side
(`PopoverPlacement::Beside`), just past the bar's surface (the popover's `band`). It stays inside
its display's work area (`bar::BarArea`, the `DockingLayout::area()` the bands leave, as the
popover's `bounds`), shifting along the bar to clear another band and scrolling when taller than
that area, so it never covers another bar. The bar's menus do the same (`bar::bar_bounds()` as the
`bounds` of the grip's `DockBarGrip`, a window chip's `ContextMenu` and a tray item's `MenuButton`),
their submenus too.

The grip menu's Hide hides a bar on its display; that display's Apps panel then offers "Show this
display's bars" (`desktop.apps.bars`), which shows its three again, and Super still opens Apps,
centred over that display's windows' area (`AppsOverlay`) while its Apps bar is hidden. Super opens
Apps on the display the pointer was last over (`beui::native::last_pointer`, its window's
`window_origin`), else the one holding the focused window, else the first. The windows' area is the dock inside its bands
(`DockingLayout::area`), where floating windows go: the level display (`desktop.osd.0`) is centred
low over it, and notification toasts gather in its corner nearest the Status bar
(`bar::toast_corner`, bottom right by default, bottom left with the bar down the left side),
below or beside the Status bar when it floats there (`bar::clear_of_status`, from
`DockingLayout::over_bars_rect`). A fullscreen window covers the bars of its display, as it covers
the whole display: the main dock's own bars drop beneath a fullscreen tab over its area, and a further
display's dock is told a focused window is fullscreen on it (`DockingLayout::set_covered`).

Display 0's bars are dock bars of the main view's dock, which holds every window. A further
display's `Display` view holds a dock of its own (`desktop.dock.{n}`) with no panes and that
display's three bars as its dock bars, on its own `DockingLayout` (`bar::display_layout`). The main
dock's state is saved in the session's layout as ever; each further display's is saved beside it,
under the session view's `display-bars` state, by the display's place in the list. A display that has
none saved starts from display 0's placements and hidden bars. Toasts stay on display 0, in the
corner nearest its Status bar.

Each display's work area is that display less its own bars' bands: the dock's area
(`DockingLayout::area`). A root dock whose windows the host shows tells its window's work area to
beui (`beui::window::set_work_area`), the plugin reports it on the view's frame report
(`frame-report.work-area`, insets), and block-app (`compositor/desktop.rs`,
`keep_in_work_areas`, `walled`) and block-ui-test's desk keep each of the plugin's windows on a
display wholly inside that display's work area on every edge where a band insets it, as the dock's
`reachable_origin` walls do for windows it draws itself; other edges, and so other displays, stay
open. A window being moved or resized is let go first, and kept once released.

The Status bar holds the tray, the device tools (the on-screen keyboard, `desktop.keyboard`,
while the display settings' `keyboard_in_bar` is on, which does not take the focus from the
field being typed in; rotate while `rotate_in_bar` is on and the host reports a screen; refresh
while the display is e-paper), then
one status button (`desktop.status`, `app/status.rs`) and the clock (`desktop.clock`,
`app/clock.rs`). The status button's face is the Wi-Fi, volume and power icons that apply
(`desktop.status.{wifi,volume,power}`), and it opens Quick settings (`desktop.quick`): tiles
for Wi-Fi (`desktop.quick.wifi`, which turns the panel to its network list, `app/wifi.rs`), the
on-screen keyboard, rotate where the display is reported and refresh on e-paper; the volume
slider, mute and output (`app/volume.rs`); and the power row (`app/power.rs`), which asks before
restarting, powering off or logging out. The clock carries the count of listed notifications
(`desktop.clock.unread`) and opens the desktop's calendar beside the notification history
(`app/notifications/history.rs`), stacked on a narrow screen; toasts make way while it is open. block-app publishes each as soon as the
service behind it runs, even empty. block-app publishes the package on Linux, and on Windows its
`windows`, `programs` and `power` (`block-app-plugin-host/src/published.rs`); a host that does not publish it (elsewhere, the web and Android)
answers every watch of the package `unsupported` and drops its calls, which the desktop
sees as nothing offered. The web and Android apps have no `--desktop`: a desktop with none of
these is only a dock of block windows, which their own shells already are.

`./scripts/buck run //crates/block-app:dev -- --desktop` runs the desktop shell headless, to
drive like any other run (guides/running/drive.md).

## Program windows

Where the host opens windows (`opens_windows()`, true only in space), every toplevel is a
window, and so a panel, of its own: workspace-ui shows each top program window as a floating dock
window (`FloatingWindow`) instead of a tab, as the desktop plugin always does. A program's dialog
(a toplevel with a parent: an xdg toplevel's parent or xdg-dialog, an X11 transient, a Windows owned
window) is never a window of its own in either shell: block-shell's `WindowDialog` declares it as a
`DockDialog` of its parent's tab, which the dock draws as the same anchored card as block-app's
own dialogs, its beak on the parent's tab in that tab's strip (or, for a dialog of a dialog, on
the parent dialog's header), escaping its pane as a layer, in space and outside it alike. Its
header, like a program window's tab and its chip, is `block_shell::window_name`, the one way to name
a host window: the program's name (`program_name`: `HostWindow::name`, else the parent's, else the
app id's last part made readable, its path and `.exe` or a package's `_publisher!App` dropped),
then " — " and the window's title when it says more (not the name or app id, a trailing
" - name" cut off). Both hosts first name a window after the program the desktop launched it from
(`origin.program`). Otherwise the Linux host fills `name` from the desktop entry named by the window's app id
(`Environment::entry`), and the Windows host from the Start menu program whose AppUserModelID or
shortcut target is the window's app id or command (guides/desktop/windows.md). A placeholder keeps
the name (`KeptWindow::name`, `KeptWindow::window_name`), and the "not responding" banner says
only the program's name. It is the size the program drew (`HostWindow::size`, followed as the
program resizes itself) and can be dragged off by its header, which drops its beak. While it is
modal, a scrim dims its parent's panel (rounded as the card when the parent is a dialog) and a press there flashes the dialog and gives it the
keyboard (see the dock in guides/beui.md); the parent's tab shows its waiting dot. A toplevel's
popups are a `Window` of the `WindowView` (`WindowRole::Popup`, so the toplevel keeps the keyboard
and the popup its grab), placed `Over` the window the toplevel is in where the overlay would be,
and opened again whenever the popups' corner moves, as when a submenu grows them leftward or
upward. The compositor still places each toplevel at its rectangle in its own window's
coordinates, and the keyboard follows the document's focus as anywhere. Outside space top windows
stay tabs, as above.

A window's and its popups' drawings paint too (`Paints` on `WindowDraw`, which reads each surface's
texture back, opaque), so paintings and `drive paint` show what programs drew.

## The Apps panel

The Apps bar holds one icon-only Apps button (`desktop.apps`, `app/apps.rs`, labelled Apps for
its tooltip and screen readers), and a tap of Super alone opens the same panel (the
`desktop.apps.toggle` action, captured from programs too, registered while the host offers
`ProgramsWatch`). The panel (`desktop.apps.panel`) is a popover of the button, and when no Apps
button shows on the display it opens on (`Apps` keeps which displays have one), as while the Apps
bar is hidden, it is centred over the windows' area instead (`AppsOverlay`). It holds, top to bottom:

- while the host offers `ProgramsWatch`, a `styled::SearchList` of the programs (`app/launcher.rs`):
  opening the panel asks the host to list them with icons at the display's scale, an empty query
  shows them as a grid of tiles (`desktop.apps.item.ID`) that the arrow keys walk, and typing
  narrows them to a list by name, keywords and comment, ending with a row that runs the query as
  a command (`desktop.apps.item.run`); choosing one launches it (`ProgramAction::Launch` or `Run`)
  and closes the panel. The watch's `ProgramCatalog::listed` is false until the host has looked for
  the programs once, so the panel says "Loading programs…" until then and "No programs found." only
  when the host found none;
- the sessions (`app/sessions.rs`): every block-app session the account's settings list, other
  than desktop sessions, each a row (`desktop.apps.sessions.UUID`, "Open" under one already in a
  window) that opens it in a floating window, or brings its window to the front if it is already
  open, and New session (`desktop.apps.sessions.new`), which adds one to the settings and opens it
  (disabled, saying why, while the workspace has no settings block);
- while one of the display's bars is hidden, "Show this display's bars" (`desktop.apps.bars`);
- Menu (`desktop.apps.menu`), which closes the panel and opens the app menu at the Apps button.

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
The desktop plugin's Windows bar has a row of window chips (`WindowChips`, `desktop.windows`,
`app/windows.rs`): one chip per toplevel entry, displays first and then windows in the host's
order, then the block windows the shell opened itself (sessions and shown blocks), each with its
icon (the window's own, else a display, block or window glyph) and its `window_name` (the program
first, so a long title is what the ellipsis cuts) ellipsised (icon only with the name as tooltip when the row is given `Direction::Vertical`). Along a horizontal bar the chips share the
length the band gives the bar, like a taskbar's: the row asks for `CHIP_WIDTH` (`Size::Menu`) a
chip, so the band gives it room as if every title were long, and the chips split what they get
evenly up to that width, their titles ellipsised; once a chip would be narrower than `CHIP_LEAST`
every chip shows only its icon, and past that the chips scroll inside the row (their own
`Scroll`, so the bar's grip stays put) and the focused chip is revealed whenever the focus, the
chip count or the icon-only switch changes. The row measures the room it was given
(`measure` on the chip list), never its own content, so switching to icons does not shrink the
room it asks for. Down a side the chips are icons already, and they scroll inside the row the same way, the grip staying at the top. The focused chip (the dock's
focused tab) is drawn pressed and a hidden one hollow. A click shows a hidden entry (`show`) and
focuses it (`focus`); a click on the focused chip hides it (`hide`, or for a block window the shell
stops declaring it) and the next shown window in the dock's focus order takes the focus. Each chip
has a context menu (secondary click, long press or Shift+F10): Hide or Show, and Close for a
window (`close`). While placeholders from last time wait, a Restore chip
(`desktop.windows.restore`) leads the row and launches them all. `WindowChips` takes a `display`
to list only the windows mostly on that display, but the plugin learns a window's place only from
its own dock state, which the host does not update when it moves a window, so the bars on every
display still list every window. Moving a window to another display is not offered: the protocol
has no request for it. On Linux it is a window
switcher too. block-app's Wayland host opens every window shown and lists no displays; `hide`
only stops the shells drawing it (the client keeps running and connected), and the shown state
lives in `block-app-desktop/src/desktop_host/shown.rs` for as long as block-app runs, forgotten when the window closes.
A hidden window shown again opens at its default place, not where it was.

## Displays

The host shows each display in a window of its own (in a login session, guides/desktop/linux.md,
Displays). The desktop is one space in points: the displays side by side along their bottom edges. Each
display's window shows its own view of the shell, macOS-like: the first display the shell's
main view (`ui::Root`'s `MainSurface`), each further one a display view (`HostView::Display(n)`) of the same instance
(`ui::SecondaryDisplay` mounts `compositor::ShellDisplay`, n being the display's place in the
list from 1), each at its display's size and scale. The shell's windows are the host's own,
placed in the desktop's points: the shell's views say the host shows the windows the plugin
opens from them (`windows` in the screen request, set for the shell's frame and display views
and for the windows themselves; guides/adding_a_plugin_editor.md), so every `Window` the shell
opens, each of the desktop plugin's dock windows, becomes a window view (`HostView::Window`) that block-app keeps
in `compositor/desktop.rs`: a list of `Floated` windows, each its rect in desktop points, in
stacking order. A press on one raises it, and so does the plugin's `raise` window request,
which the dock sends when a window takes the focus (a press on a program's content focuses the
program, which focuses its tab); raising a window brings the windows opened over it, its dialogs
and popups, along in front of it, and moving one (a drag or a `place` request) moves them with it. The dock's windows clear to transparent, so the margin their
resize grips reach into and their rounded corners show what is behind them. The shell's view on each display draws, between its own
surface and its floating layers (menus, popovers, toasts), the windows that display holds most
of (`desktop::shown_on`, `DesktopWindows` in `compositor/plugin_view.rs`), each a `PluginView` of
its window view placed at its rect less the display's corner, whose children are the program
windows and block editors the shell placed in it. A window opens where the plugin asked: at a
point of the view it opened from (`Over`, measured from that view's corner on the desktop),
at the pointer, or at the `placement` it kept (`desktop X Y`); the host answers with that
placement on the window's screen request, which the dock keeps. Every screen request of the shell's
frame, display and window views also carries the view's `origin`, its top-left corner in desktop
points, asked again whenever it changes; beui-platform-plugin reports it as each window's
`WindowReport::Moved`, and the dock keeps a window's rect (`DockingLayout::window_rect`, in its
own window's points) from it, so it stays true after the host moves a window, and when a dock bar
beside its top or left edge moves the dock's own area. A dock window whose
rect the dock changes itself (`DockingLayout::move_window`) asks the host for its new place with
the protocol's `place` window request (beui's `WindowRequest::Place`, an `Over` its home view).
The Windows bar on each display lists the windows mostly on that display, and a chip's menu
offers "Move to display N" (`<chip>.display.<n>`, `app/windows.rs`), which keeps the window's size
and its relative place on the display, held inside it. `BeginMove` and `BeginResize`
hand the press to the host: the window follows the pointer from where it was pressed (the
press's point and the window's rect are noted on the press, `desktop::follow`), the plugin sees
no moves meanwhile, and the release ends it and gives the new placement. In the app's own window (`OverlaidWindows`, no desktop) a window the user has moved is kept where its top-left corner, its left 96 points and the point it was held by stay in view (`desktop::reachable`), pulled past that it stretches like a rubber band and springs back when let go (`desktop::carried`), and one never moved is kept inside the app, shrinking rather than covering what opened it (`desktop::clamped`). While one moves it is
drawn on every display it overlaps and on the one the move began on, which holds the press;
once let go it is drawn only on the display holding most of it. Each view is laid out at the
document's scale and rendered at the raster of the display that shows most of it (each mount's
`EmbedPlacement::raster`, `ViewPlace::raster` of the offer whose clip is largest), so a window moved onto a display at another scale is rendered at that scale once
it is mostly there; while it straddles two, the display that holds less of it shows the same
texture scaled. Program windows and block editors are children of the windows they are tabs
of, so they stack and take the focus with them. The pointer crosses between display windows
with a press held, by beui's cross-window rules (the platform reports each window's desktop
origin, `WindowReport::Moved`), and between the shell's views by the protocol's `over`: a
press held in one of the shell's views names the view the pointer is over, so a tab dragged
out of one window and over another, on any display, lands on that window's bar.

`--desktop` in a window, or headless, is the same with one display, the window: the shell's frame
fills it, and its windows are the host's own, drawn over it in that window's points. The plugin
tests do the same through block-ui-test's `BeuiTest::show_on_displays`, which shows the views
and hosts the windows as block-app does, gives each display and window an anchor space so
popovers and dialogs escape above the windows, takes input in desktop points (`desk_press`,
`desk_move`, `desk_release`, `desk_drag`; `click`, `hover`, `down`, `up` and `drag` go the same
way, a test id to the centre of the first place the desk shows it, revealed first when a scroll cuts it, a point read in desktop
points), sends `app_tap` to whichever view holds the keyboard (the one whose window requests
intercept the tap) and paints a display with the windows and escaped layers on it
(`desk_snapshot`); without it a plugin draws its windows inside its frame.

The shell learns the displays from `DisplaysWatch`: each `HostDisplay` carries its `rect` in the
desktop's points and its `scale`. The desktop plugin draws, in every view, a level display (`desktop.osd.{n}`, n the view's
display), a dock with that display's three bars and its Apps overlay, and in the frame alone the
windows, pickers and notification toasts; `BeuiApp::Display` draws the further displays' views
from what the frame's view shares (`app.rs`, `Desktop`). The dock fills the frame,
and its windows open over it.

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
panel from the Apps panel's Menu and press Crash beside `be3.desktop`.

## A layout that cannot be read

When the saved layout (the `layout` view state) does not decode, the desktop starts nothing from it
and saves nothing over it: block-shell's notice (`workspace.layout`, see guides/the_new_block_stack.md,
View state) offers Reset layout, Exit and Copy report, and every window and placeholder it named stays
in the saved bytes until the person resets. Exit asks the host to close the shell itself; in a login
session that is Log out (programs are asked to close, then block-app exits and the session ends),
elsewhere block-app closes as its window's close button does. The programs already running are shown
behind the notice as live windows, but are not saved until a reset.
