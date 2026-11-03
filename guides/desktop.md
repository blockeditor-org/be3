# The desktop

be-host is also a desktop: its shell is the desktop app (`crates/be-app-desktop`, app
id `be3.desktop`) instead of workspace-ui, on a session of its own, and the host shows other
programs' windows inside it. This guide is what every host shares; each host has its own:

- guides/desktop/linux.md: the Linux host, a Wayland compositor (be-host-wayland) that also runs
  on the displays themselves as a login session (`BEUI_PLATFORM=drm`) and installs as one.
- guides/desktop/windows.md: the Windows host, which shows Windows' monitors and windows.

`--desktop` (every native platform) starts the desktop shell in a normal window. On Linux
programs open as windows inside it (guides/desktop/linux.md); on Windows it shows Windows'
monitors and windows, launches Start Menu programs and offers power (guides/desktop/windows.md);
on macOS (`src/arguments.rs` parses the flags) it is the shell alone for now: there are no program
windows, notification server, tray, media, power, lock screen or programs to launch.
The host test `the_desktop_shell_opens_and_closes_cleanly` (crates/be-host-tests) starts it headless
and closes it. Only `--desktop` hosts other programs' windows: `HostCore::desktop`, set by
`run_as_desktop`, is what makes be-host create and start `DesktopHost` (Linux's Wayland compositor and
Xwayland, Windows' window watcher), and `be_host_runtime::run_as_desktop` is what offers the
package's `windows` and `programs` (`published::offers`). A run without it, on the displays (`drm`)
or in space included, starts neither, answers those two watches `unsupported` and so lists no
program windows, no programs to launch and no program as an editor of a block. `BEUI_PLATFORM=drm` and `--install-login-session` are Linux's (guides/desktop/linux.md).
A desktop never shares its data with another be-host: started while one already runs on the same
data directory, it stops with an error (guides/running/native.md).

Everything the desktop gets from the host comes through `be3:desktop`, a WIT package
beside the core app protocol rather than in it
(`crates/be-sdk-api/wit/deps/desktop`): one interface per part of the session
(`windows`, `input`, `displays`, `power`, `media`, `wifi`, `epaper`, `notifications`,
`screen-lock`, `programs`, `tray`, sharing `types`' `host-image`), each a latest-value watch
(`WindowsWatch`, `PowerWatch`, ...) and, where the desktop asks something of the host, a
function (`power::request(PowerAction)`, `epaper::refresh()`, ...). Another host, a compositor
in another language, can implement it from the WIT alone. An app uses it only by naming
`be3:desktop` in its manifest's `extensions` (the desktop app, workspace-ui for the program
windows, and settings for displays, input devices and remaps); the host denies a watch or a call
from any other. The desktop app shows a control only while the host offers what backs it
(`editor.offered::<W>()`, true once the host answered the watch): the Apps panel's program
search and grid, the Super tap and placeholders' Open need `ProgramsWatch`, the clock's notification count and
history `NotificationsWatch`, the power row `PowerWatch`, the media keys `MediaWatch` (the
volume icon and controls, its `output`), the Wi-Fi icon and tile `WifiWatch` (its `available`),
the keyboard layout indicator and Super+Space `KeyboardLayoutsWatch` (more than one layout), and the
tray its items.
Settings shows each input and display setting where it takes effect, not wherever one
coarse watch is answered: the keymap and key repeat while `InputReachWatch`'s `keyboard`
is (the keyboards the seat reads, under DRM, or the programs any desktop hosts), middle-click
paste while its `middle_click_paste` is (any desktop's programs), the pointers and remaps only
while `InputDevicesWatch` is (a seat of physical devices), the bar's keyboard button while
`DesktopBarWatch` is (any desktop) and its rotate button while that and `DisplaysWatch` are.
be-host publishes `InputReachWatch` from `input::start` and `DesktopBarWatch` from
`run_as_desktop`, so a windowed desktop sets its programs' keymap too.

## The bars

The shell is three dock-level `DockBar`s of the desktop's own dock (`desktop.dock`), declared in
`DesktopBody` (`root/bar.rs`), each a `DockBarSurface` with a `DockBarGrip` first:

- Apps (`desktop.bar.apps`, `ProgramsBar`): the Apps button (The Apps panel, below).
- Windows (`desktop.bar.windows`, `WindowsBar`): the window chips (Shown and hidden entries).
- Status (`desktop.bar.status`, `StatusBar`): the tray, the device tools, the status button and
  the clock.

Every display has its own three (`bar::Bars::of(display)`: be-sdk-shell's `SHELL_BARS + 3 * display`
and the two after it), which it places and hides by itself. By default all three sit beside the
bottom edge in one band (`dock.bar.band.bottom`): Apps at its start, Windows centred, Status at its
end. A grip moves its bar like any dock bar: beside any edge at its start, centre or end, floated over
one, or popped out into a window; the dock saves where each is and whether it is hidden, in the
session's layout. The host maps the desktop's windows into the displays' one space, so dragging a
popped-out bar's grip lifts the bar out of its window to follow the pointer and land like any other
dock bar (guides/beui.md, the dock's bars); it does not move the window. Every part lays itself out along its bar (`bar::bar_direction`, from
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
centred over that display's windows' area (`ProgramsPanelOverlay`) while its Apps bar is hidden. Super opens
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
under the session view's `display-bars-by-monitor` state as a `SavedDockLayout` naming display 0's bar ids
(`SavedDockLayout::map_tabs`, so it fits whichever place the display is in), keyed by the
`HostDisplay::id` at that display's place (its EDID make, model and serial, or else its connector),
so a display keeps its own bars when displays are unplugged or change places: when the id at a
place changes, that place's dock restores the new display's bars. A display that has
none saved starts from display 0's placements and hidden bars. State that does not decode is
treated as the main layout is (below): every further display starts from display 0's bars, nothing
is saved over the bytes, and display 0 shows a notice (`desktop.display-bars`, while more than one
display is connected) with Copy report, Keep them and Reset bars. Toasts stay on display 0, in the
corner nearest its Status bar.

Each display's work area is that display less its own bars' bands: the dock's area
(`DockingLayout::area`). A root dock whose windows the host shows tells its window's work area to
beui (`beui::window::set_work_area`), the app reports it on the view's frame report
(`frame-report.work-area`, insets), and be-host (`compositor/desktop.rs`) and be-sdk-test's
desk, through be-sdk-window-stack's `keep_in_work_areas` and `fitted`, keep each of the app's windows on a
display wholly inside that display's work area on every edge where a band insets it, shrinking one
too big for it, as the dock's `reachable_origin` walls do for windows it draws itself; other edges,
and so other displays, stay open. A window on no display, such as one whose display was unplugged,
belongs to the nearest (`be_sdk_window_stack::home`) and is brought wholly into its work area. A
window being moved or resized is let go first, and kept once released. Under DRM the displays are
`display::shown()`; in a window or headless they are one, be-host's window, which the shell's main
view holds (`desktop::hold_window_display`). The shell opens a program's window, and a restored
placeholder, on a cascade inside its dock's area (`cascaded` in be-sdk-shell's `window.rs`), with
fewer steps when the window leaves little room. A window opens within the least and greatest size
its program asks for (`min-size` and `max-size` on `host-window`, `within_limits`), shrunk to the
dock's area, at its origin, when the least does not fit. Only the opening size honours them: the
person may resize the window past them, and the window does not follow a size its program commits
that is only those limits applied to the size it was given (`followed_size`).

The Status bar holds the tray, the keyboard layout indicator (`desktop.layout`, guides/desktop/linux.md),
the device tools (the on-screen keyboard, `desktop.keyboard`,
while the display settings' `keyboard_in_bar` is on, which does not take the focus from the
field being typed in; rotate while `rotate_in_bar` is on and the host reports a screen; refresh
while the display is e-paper), then
one status button (`desktop.status`, `root/status.rs`) and the clock (`desktop.clock`,
`root/clock.rs`). The status button's face is the Wi-Fi, volume and power icons that apply
(`desktop.status.{wifi,volume,power}`), and it opens Quick settings (`desktop.quick`): tiles
for Wi-Fi (`desktop.quick.wifi`, which turns the panel to its network list, `root/wifi.rs`), the
on-screen keyboard, rotate where the display is reported and refresh on e-paper; the volume
slider (to `MediaLevel::LOUDEST`, 150%, with full marked; the volume keys stop at full), mute
and output (`root/volume.rs`); and the power row (`root/power.rs`), which asks before
restarting, powering off or logging out, and gives each action `PowerAvailability` does not
allow its reason as the button's description (the session is ending, the host does not run the
login session, which `session` says, or the computer does not allow it). The clock carries the count of listed notifications
(`desktop.clock.unread`) and opens a month (beui's `Calendar`, today marked) with the chosen day's
events beside the notification history (`root/notifications/history.rs`), stacked on a narrow
screen (`root/calendar.rs`). The events come from the desktop's own calendar, a database calendar
view it creates once and remembers in its `desktop.calendar` view state; Open calendar
(`desktop.calendar.open`) opens that view in a window of its own, the popup never embeds it; toasts make way while it is open. be-host publishes each as soon as the
service behind it runs, even empty. be-host publishes the package on Linux, its `windows` and `programs` only under `--desktop`,
and on Windows only under `--desktop`, its `windows`, `programs` and `power`
(`be-host-runtime/src/published.rs`); a host that does not publish it (elsewhere, the web and Android)
answers every watch of the package `unsupported` and drops its calls, which the desktop
sees as nothing offered. be-host reaches the host through
`be_host_desktop::desktop_host::Host`, the platform's `DesktopHost`: a trait of associated
functions (start, list and act on windows, launch) whose defaults do nothing, so a platform with
no host (`impl DesktopHost for Host { type Source = (); }`) writes none of them. Its `Source` is
the `ProgramSource` that lists the installed programs, their icons and openers and runs one;
`programs/installed.rs` is the one `Programs<S>` over it that scans on a thread, holds launches
until the list is in and publishes `ProgramsWatch`. The scan's results reach the UI thread through
`be_host_base::host::mailbox`, whose handler runs when the frame starts, so nothing polls for
them.

That mailbox is the one way any work in be-host hands a result back to the UI thread: a worker
`Post::send`s a value, which wakes the event loop, and the handler runs on the thread that made
the mailbox. Dropping the `Mailbox` cancels it: whatever is still on its way is dropped. A handler
that needs `&mut` state its owner holds is made on a `host::Desk<C>`, and runs, given the
`&mut C`, when the owner calls `desk.deliver(&mut c)`: `HostCore`'s account, organization, key,
member and contact requests and logind's events go through its desk at the start of its frame
(`request(work, handler)` in main.rs spawns one), each app's `Instances` delivers head reads,
audio changes and service answers on its own, and so do `Programs` and `ScreenLock`. The web and
Android apps have no `--desktop`: a desktop with none of
these is only a dock of block windows, which their own shells already are.

`./scripts/buck run //crates/be-host:dev -- --desktop` runs the desktop shell headless, to
drive like any other run (guides/running/drive.md).

## Program windows

Only the shell sees the program windows: the host answers `WindowsWatch` and takes its
`windows::request` only from the shell instance, so a session opened inside the desktop is a block
window and shows none of them. Where the host opens windows (`opens_windows()`, true only in space), every toplevel is a
window, and so a panel, of its own: be-sdk-shell's `Workspace` shows each top program window as a
floating dock window (`FloatingWindow`) instead of a tab, as the desktop app always does. A program's dialog
(a toplevel with a parent: an xdg toplevel's parent or xdg-dialog, an X11 transient, a Windows owned
window) is never a window of its own in either shell: be-sdk-shell's `WindowDialog` declares it as a
`DockDialog` of its parent's tab, which the dock draws as the same anchored card as be-host's
own dialogs, its beak on the parent's tab in that tab's strip (or, for a dialog of a dialog, on
the parent dialog's header), escaping its pane as a layer, in space and outside it alike. A
parent can come and go after the window shows (GTK 3 sets it after the first commit), and the
dialog moves between being a window and a card with it. A parent that is not listed (a toplevel
that never drew, such as `gtk3-demo --run`'s hidden main window) does not count: be-host-wayland
reports the nearest shown ancestor (`State::parent`), and `shown_windows` drops a parent missing
from the list, so such a window is a window of its own. Its
header, like a program window's tab and its chip, is `be_host_desktop::naming::window_name` (be-sdk-shell's
`window_name` is a copy the shells use until they read the window child's `title`), the one way to name
a host window: the program's name (`program_name`: `HostWindow::name`, else the parent's, else the
program id's last part made readable, its path and `.exe` or a package's `_publisher!App` dropped),
then " — " and the window's title when it says more (not the name or program id, a trailing
" - name" cut off). Both hosts first name a window after the program the desktop launched it from
(`origin.program`). Otherwise the Linux host fills `name` from the desktop entry named by the window's program id
(`Environment::entry`), and the Windows host from the Start menu program whose AppUserModelID or
shortcut target is the window's program id or command (guides/desktop/windows.md). A placeholder keeps
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

A placed `SubregionContent::Window` child, and a program opened to edit a block (Files opened in
programs, below), is drawn by be-host-desktop's `PlacedWindow`: the platform's `WindowSurface` inside
`WindowBody` (`window_body.rs`), which fades the edges where the program's latest buffer reaches past
the child's rect (beui's scroll fade), says "Too small for" the program while the rect is smaller
than the program's least size, and draws "{name} is not responding" with Wait and Force close
(`host.window.not_responding`, `host.window.wait`, `host.window.force_close`) over the body. The
child's status comes from `DesktopHost::window_status` (`WindowStatus`): its title (the name
above), icon, group (its program id), raised, rung, fullscreen (the program asks), notice, menu-asked
and intrinsic (the size the program picked). be-host's `close-child` closes the window as
`window-action.close` does, a change of `child-placement.fullscreen` asks the program to enter or
leave full screen, and the keyboard the placer gives the child, or its `focus-child`, gives the
window the keyboard (`follow_window` in be-host's `compositor/editors.rs`).

A window's and its popups' drawings paint too (`Paints` on `WindowDraw`, which reads each surface's
texture back, opaque), so paintings and `drive paint` show what programs drew.

## The Apps panel

The Apps bar holds one icon-only Apps button (`desktop.apps`, `root/programs_panel.rs`, labelled Apps for
its tooltip and screen readers), and a tap of Super alone opens the same panel (the
`desktop.apps.toggle` action, captured from programs too, registered while the host offers
`ProgramsWatch`). The panel (`desktop.apps.panel`) is a popover of the button, and when no Apps
button shows on the display it opens on (`ProgramsPanel` keeps which displays have one), as while the Apps
bar is hidden, it is centred over the windows' area instead (`ProgramsPanelOverlay`). It holds, top to bottom:

- while the host offers `ProgramsWatch`, a `styled::SearchList` of the programs (`root/launcher.rs`):
  opening the panel asks the host to list them with icons at the display's scale, an empty query
  shows them as a grid of tiles (`desktop.apps.item.ID`, a tooltip giving a name cut short in
  full) that the arrow keys walk, starting from the first each time the panel opens, and typing
  narrows them to a list by name, keywords and comment, ending with a row that runs the query as
  a command (`desktop.apps.item.run`), in an area of a fixed height once the host lists any
  programs, so the search field stays put as the results narrow; choosing one launches it (`ProgramAction::Launch` or `Run`)
  and closes the panel. The watch's `ProgramCatalog::listed` is false until the host has looked for
  the programs once, so the panel says "Loading programs…" until then and "No programs found." only
  when the host found none;
- the sessions (`root/sessions.rs`): every be-host session the account's settings list, other
  than desktop sessions, each a row (`desktop.apps.sessions.UUID`, "Open" under one shown in a
  window, "Hidden" under one whose window is hidden) that opens it in a floating window, or shows
  its window and brings it to the front with the focus if it is already open, and New session (`desktop.apps.sessions.new`), which adds one to the settings and opens it
  (disabled, saying why, while the workspace has no settings block);
- while one of the display's bars is hidden, "Show this display's bars" (`desktop.apps.bars`);
- Menu (`desktop.apps.menu`), which closes the panel and opens be-host's menu at the Apps button.

## Notes

`HostWindow::note` is what the host wants said about a window right now, framework-neutral text.
be-sdk-shell's `WindowPanel` hands it to its dock tab (`DockTabControl::show_notice`), so the
window's tab or dialog header shows it in place of the title while it is set; nothing is drawn
over the window's content for it. The Windows host sets it for focus mode, hovering and refused
input (guides/desktop/windows.md); the Linux host sets none.

## Shown and hidden entries

`WindowsWatch` lists every entry the host could show: program windows (`kind: window`) and, on a
host that captures them (Windows), whole displays (`kind: display`, `size` the display's, `title`
its name). Each has `shown`, which the host keeps, so a shell started again shows what was shown.
The shells draw only shown entries: be-sdk-shell's `Workspace` keeps the whole list
(`Workspace::entries`) and lays out `shown_windows` of it, so a hidden window is no tab, no
floating window and no saved window, and a dialog whose toplevel is hidden is hidden with it.
The desktop app's Windows bar has a row of window chips (`WindowChips`, `desktop.windows`,
`root/windows.rs`): one chip per toplevel entry, displays first and then windows in the host's
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
focuses it (`focus`); a click on the focused chip hides it (`hide`; a block window's tab goes into the
`Workspace`'s hidden set, `Workspace::hide`, and the shell stops declaring it, and
`Workspace::show` or opening its block again, from the Apps panel or a show request, shows it) and the next shown window in the dock's focus order takes the focus. Each chip
has a context menu (secondary click, long press or Shift+F10): Hide or Show, and Close for a
window (`close`). A chip is drawn as a primary button for `BELL_FLASH` each time its window's program rings the bell (`HostWindow::rung` counts up). With no chip and nothing to restore, a horizontal row shows a muted "No windows" a chip wide
(`desktop.windows.none`), so an empty bar keeps a taskbar's length. While placeholders from last time wait, a Restore chip
(`desktop.windows.restore`) leads the row and launches them all. `WindowChips` takes a `display`
to list only the windows mostly on that display (`display_of`: the display holding most of the
window's rect in the dock, which follows each window's reported `origin`, Displays below; a hidden
window, which has no rect, is listed on the first display), and with more than one display a
shown floating window's chip menu adds "Move to display N" for each other display (Displays,
below). On Linux it is a window switcher too. be-host's Wayland host opens every window shown and lists no displays; `hide`
only stops the shells drawing it (the client keeps running and connected), and the shown state
lives in `be-host-desktop/src/desktop_host/shown.rs` for as long as be-host runs, forgotten when the window closes.
A hidden window shown again opens at its default place, not where it was.

## Displays

The host shows each display in a window of its own (in a login session, guides/desktop/linux.md,
Displays). The desktop is one space in points: the displays as arranged in Settings > Display, or
else side by side along their bottom edges. Each
display's window shows its own view of the shell, macOS-like: the first display the shell's
main view (`ui::Root`'s `MainSurface`), each further one a display view (`HostView::Display(n)`) of the same instance
(`ui::SecondaryDisplay` mounts `compositor::ShellDisplay`, n being the display's place in the
list from 1), each at its display's size and scale. The shell's windows are the host's own,
placed in the desktop's points: the shell's views say the host shows the windows the app
opens from them (`windows` in the screen request, set for the shell's frame and display views
and for the windows themselves; guides/adding_an_app.md), so every `Window` the shell
opens, each of the desktop app's dock windows, becomes a window view (`HostView::Window`) that be-host keeps
in `compositor/desktop.rs`: a list of `Floated` windows, each its rect in desktop points, in
stacking order. The window manager itself, without signals, is the `be-sdk-window-stack` crate: the
app's window requests in (`ask`), stacking (`raise`), moves and resizes (`move_in`, `drag`,
`resized`), work areas and the `desktop X Y` placement; be-host wraps it in signals and
be-sdk-test's desk (`show_on_displays`) runs it directly, so the app tests drive the same one. A press on one raises it, and so does the app's `raise` window request,
which the dock sends when a window takes the focus (a press on a program's content focuses the
program, which focuses its tab); raising a window brings the windows opened over it, its dialogs
and popups, along in front of it, and moving one (a drag or a `place` request) moves them with it. The dock's windows clear to transparent, so the margin their
resize grips reach into and their rounded corners show what is behind them. The shell's view on each display draws, between its own
surface and its floating layers (menus, popovers, toasts), the windows that display holds most
of (`desktop::shown_on`, `DesktopWindows` in `compositor/app_view.rs`); the dock's bands of
bars are among those floating layers, as on other desktops, so a window carried over a bar
passes beneath it and the bar still takes presses (beui-dock's `DockBarBand` marks its band
`AboveEmbeds::Paints` while `bar_level` is `Raised`, so the view's frame report lists it in
`floating` and `floating-painted`), and drop beneath a fullscreen window as `bar_level` does.
A bar popped out into a window of its own opens at `WindowLevel::Above` (the window-open's
`level`) while `bar_level` is `Raised`: be-sdk-window-stack's `settle`, run after every open and
raise, keeps such windows and the windows opened over them after the normal ones in the list, so a
raised program window passes beneath the bar, still under the view's floating layers and the
lock screen; a fullscreen window drops the bar to `Normal` and so covers it once raised.
Each window is a `AppView` of
its window view placed at its rect less the display's corner, whose children are the program
windows and block editors the shell placed in it. A window opens where the app asked: at a
point of the view it opened from (`Over`, measured from that view's corner on the desktop),
at the pointer, or at the `placement` it kept (`desktop X Y`); the host answers with that
placement on the window's screen request, which the dock keeps. Every screen request of the shell's
frame, display and window views also carries the view's `origin`, its top-left corner in desktop
points, asked again whenever it changes; be-sdk-beui-platform reports it as each window's
`WindowReport::Moved`, and the dock keeps a window's rect (`DockingLayout::window_rect`, in its
own window's points) from it, so it stays true after the host moves a window, and when a dock bar
beside its top or left edge moves the dock's own area. A dock window whose
rect the dock changes itself (`DockingLayout::move_window`) asks the host for its new place with
the protocol's `place` window request (beui's `WindowRequest::Place`, an `Over` its home view).
The Windows bar on each display lists the windows mostly on that display, and a chip's menu
offers "Move to display N" (`<chip>.display.<n>`, `root/windows.rs`), which keeps the window's size
and its relative place on the display, held inside it. `BeginMove` and `BeginResize`
hand the press to the host: the window follows the pointer from where it was pressed (the
press's point and the window's rect are noted on the press, `desktop::follow`), the app sees
no moves meanwhile, and the release ends it and gives the new placement. In be-host's own window (`OverlaidWindows`, no desktop) a window the user has moved is kept where its top-left corner, its left 96 points and the point it was held by stay in view (`desktop::reachable`), pulled past that it stretches like a rubber band and springs back when let go (`desktop::carried`), and one never moved is kept inside be-host's window, shrinking rather than covering what opened it (`desktop::clamped`). While one moves it is
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
fills it, and its windows are the host's own, drawn over it in that window's points. The app
tests do the same through be-sdk-test's `BeuiTest::show_on_displays`, which shows the views
and hosts the windows as be-host does, gives each display and window an anchor space so
popovers and dialogs escape above the windows, takes input in desktop points (`desk_press`,
`desk_move`, `desk_release`, `desk_drag`, and `desk_moves`, which advances the clock by a given
step before each move so a fluid drag is followed frame by frame; `click`, `hover`, `down`, `up` and `drag` go the same
way, a test id to the centre of the first place the desk shows it, revealed first when a scroll cuts it, a point read in desktop
points), sends `host_tap` to whichever view holds the keyboard (the one whose window requests
intercept the tap) and paints a display with the windows, the floating layers the display reports (taking their presses) and the escaped layers on it
(`desk_snapshot`); without it an app draws its windows inside its frame.

The shell learns the displays from `DisplaysWatch`: each `HostDisplay` carries its `rect` in the
desktop's points and its `scale`. The desktop app draws, in every view, a level display (`desktop.osd.{n}`, n the view's
display), a dock with that display's three bars and its Apps overlay, and in the frame alone the
windows, pickers and notification toasts; `BeuiRoot::Display` draws the further displays' views
from what the frame's view shares (`root.rs`, `DesktopRoot`). The dock fills the frame,
and its windows open over it.

## Window keys

The window manager is beui-dock's (guides/beui.md, the dock's window actions), over the desktop's
dock and each display's work area in the main dock's points: `DeskWindows` gives display 0 the
dock's `area()` and every further display its `Display` view's area (`DeskWindows::report_area`)
and passes them to `DockingLayout::set_areas`. It acts on program and block windows alike, as
floating dock windows. be-sdk-shell's `BlockTab` and `WindowTab` give their tabs a `group`
(`block:` and the block type, `program:` and the program id) for the app's next window, and
`Workspace::hide` and `show` go through the dock. Until program windows are the host's children
(plans/program_windows.md), the host still keeps whether a program window is shown:
`Workspace::mirror_shown` keeps the dock's hidden windows and the host's `shown` in step both ways
(the desktop lists hidden program windows too, `listed_windows`, so their dock windows stay). The
desktop app binds keys to the dock's `window_actions`, disabled while `ScreenLockedWatch` is set
(`root/window_keys.rs`, at `KeyAt::Window` and capturing, so they work over programs and are listed
in the command palette); workspace-ui binds none, since its windows are its own:

| Action | Linux | Where the system keeps Super |
| --- | --- | --- |
| Close window | Alt+F4 | Ctrl+F4 |
| Snap to the left or right half | Super+Left, Super+Right | Ctrl+Alt+Left, Ctrl+Alt+Right |
| Maximize | Super+Up | Ctrl+Alt+Up |
| Restore a snapped window, else hide it | Super+Down | Ctrl+Alt+Down |
| Show desktop | Super+D | Ctrl+Alt+D |
| Move to the next or previous display | Super+Shift+Right, Super+Shift+Left | Ctrl+Alt+Shift+Right, Ctrl+Alt+Shift+Left |
| The program's next window | Super+` | Ctrl+Alt+` |
| Full screen (the tab's, which a program window's tab asks of the program) | Super+F | Ctrl+Alt+F |
| Window switcher, forwards and back | Alt+Tab, Alt+Shift+Tab | the same |
| Leave the switcher | Alt+Escape | the same |

The system keeps Super's chords where the host answers `SuperKeptWatch` true, as be-host does on
Windows, whose own shell takes Win+arrows and Win+D for be-host's window and where Alt+F4 closes
be-host itself. Super+Left snapping the other way restores, as does Super+Right; a key whose
action has nothing to do (Super+Up on a maximized window) is not taken, so it reaches the program.
A Super chord is never a Super tap, so none opens the Apps panel.

## A shell that stops

When the shell's app stops (a panic, a trap, a hang, its session failing or ending, its
presenter failing, or Kill in the Apps panel), be-host's `watch_shell` closes it and every editor under it,
drops its runtime and shows the host's own session and profile picker (`Screen::Sessions`),
with the error toast and its Copy report when it crashed. Every stop goes through
`Runtime::stopped`, which shuts the app down and reports its id once to `take_stopped`; a
runtime dropped while still running (`Host::shutdown`) reports there too. Choosing a session or profile, or keeping the current
one, opens a fresh shell. The windows stay. On Linux the compositor keeps every client connected,
and `Compositor::set_owned(false)`, which be-host sets whenever no shell is on screen, takes
the keyboard, the pointer, held buttons and popups from them, drops any input meant for them
and pings every client each `PING_TIMEOUT`, so one that froze meanwhile is listed as not
responding. The lock screen is the host's and works throughout. The new shell sees the
windows in `WindowsWatch` and shows them as it shows any window; the desktop app's saved layout
names window tabs by `HostWindowId`, which the host keeps, so a restored layout puts
each window back where it was, fullscreen and dialogs included. To try it, open be-host's
menu (the Apps panel's Menu row), choose Running Apps, be-host's debug panel of the apps it runs,
and press Crash or Kill beside `be3.desktop`.

## A layout that cannot be read

When the saved layout (the `layout` view state) does not decode, the desktop starts nothing from it
and saves nothing over it: be-sdk-shell's notice (`workspace.layout`, see guides/the_new_block_stack.md,
View state) offers Reset layout, Exit and Copy report, and every window and placeholder it named stays
in the saved bytes until the person resets. Exit asks the host to close the shell itself; in a login
session that is Log out (programs are asked to close, then be-host exits and the session ends),
elsewhere be-host closes as its window's close button does. The programs already running are shown
behind the notice as live windows, but are not saved until a reset.
