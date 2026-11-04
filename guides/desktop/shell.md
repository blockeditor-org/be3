# The desktop shell

The desktop app (`crates/be-app-desktop`, `src/root/`) is the shell under `--desktop`
(guides/desktop.md). It is a beui app over be-sdk-shell's `Workspace`, with a dock (`desktop.dock`)
whose floating windows hold program and block tabs. Everything it shows from the host comes from
`be3:desktop` watches, and each control shows only while its watch is offered.

Test it in the app's own tests (`src/tests/`) with be-sdk-test's desk
(`BeuiTest::show_on_displays`, guides/desktop.md, Displays), and in the real host with
`./scripts/buck run //crates/be-host:dev -- --desktop` and `drive`.

## The bars

The shell is three dock-level `DockBar`s per display, declared in `DesktopBody` (`root/bar.rs`),
each a `DockBarSurface` with a `DockBarGrip`:

- Apps (`desktop.bar.apps`, `ProgramsBar`): the Apps button.
- Windows (`desktop.bar.windows`, `WindowsBar`): the window chips.
- Status (`desktop.bar.status`, `StatusBar`): tray, keyboard layout, device tools, status button,
  clock.

By default all three share the bottom band. A grip moves a bar like any dock bar (beside an edge,
floated, or popped out into a window; guides/beui/docking.md, the dock's bars), and the dock saves where
each is and whether it is hidden. Each part lays itself out along its bar's direction
(`bar::bar_direction`): down a side, chips show only icons and the clock stacks.

Things that place relative to the bars:

- A bar button's panel (`BarPopup`) opens away from the bar's edge, kept inside the display's work
  area (`bar::BarArea`, passed as the popover's `bounds`) so it never covers another bar. The bars'
  menus use `bar::bar_bounds()` the same way.
- Toasts gather in the corner of the windows' area nearest the Status bar (`bar::toast_corner`,
  `bar::clear_of_status`). The level display (`desktop.osd.{n}`) is centred low on each display.
- A fullscreen window covers its display's bars (`DockHostLayout::set_covered` for further
  displays).

**Displays.** Bar ids are `bar::Bars::of(display)` (be-sdk-shell's `SHELL_BARS + 3 * display`).
Display 0's bars are dock bars of the main dock. Each further display's `Display` view holds a
pane-less dock of its own (`desktop.dock.{n}`, `bar::display_layout`), saved under the session
view's `display-bars-by-monitor` state as a `SavedDockLayout` naming display 0's bar ids
(`SavedDockLayout::map_tabs`), keyed by the display's `HostDisplay::id` (EDID, else connector), so a
monitor keeps its bars when displays are unplugged or reordered. One with none saved starts from
display 0's. State that does not decode is not saved over; display 0 shows a notice
(`desktop.display-bars`) with Copy report, Keep them and Reset bars.

Hiding a bar (grip menu > Hide) leaves "Show this display's bars" (`desktop.apps.bars`) in the
Apps panel, and Super still opens the panel, centred over the windows' area
(`ProgramsPanelOverlay`).

## The Status bar

- Tray items (`root/tray.rs`, guides/desktop/linux_services.md).
- Keyboard layout indicator (`desktop.layout`, `root/layouts.rs`) while there is more than one
  layout.
- Device tools, each behind a display setting: on-screen keyboard (`desktop.keyboard`,
  `keyboard_in_bar`; it does not take the focus from the field), rotate (`desktop.rotate`,
  `rotate_in_bar`, when the host reports a screen) and refresh on e-paper.
- The status button (`desktop.status`, `root/status.rs`): the Wi-Fi, volume and power icons that
  apply. It opens Quick settings (`desktop.quick`): tiles for Wi-Fi (`root/wifi.rs`), the
  on-screen keyboard, rotate and refresh; the volume slider up to `MediaLevel::LOUDEST` (150%) with
  mute and output (`root/volume.rs`); and the power row (`root/power.rs`), which confirms restart,
  power off and log out and shows `PowerAvailability`'s reason on each action it does not allow.
- The clock (`desktop.clock`, `root/clock.rs`) with the unread notification count; it opens a
  calendar month with the day's events beside the notification history (`root/calendar.rs`,
  `root/notifications/history.rs`). The events come from a database calendar view the desktop
  creates once and remembers in its `desktop.calendar` view state. Toasts make way while it is open.

While the session is ending, the ending bar (`root/ending.rs`) names the programs still open, with
Cancel and End now (guides/desktop/login_session.md, Power and logging out).

## The Apps panel

The Apps button (`desktop.apps`, `root/programs_panel.rs`) and a tap of Super alone
(`desktop.apps.toggle`, registered while the host offers `ProgramsWatch`) open the panel
(`desktop.apps.panel`). Super opens it on the display the pointer was last over
(`beui::native::last_pointer`), else the focused window's, else the first; with no Apps button on
that display it is centred over the windows' area. It holds:

- the program search (`root/launcher.rs`, a `styled::SearchList`), while `ProgramsWatch` is
  offered: a grid of tiles for an empty query (`desktop.apps.item.ID`), a list narrowed by name,
  keywords and comment otherwise, ending with a row that runs the query as a command
  (`desktop.apps.item.run`). It keeps a fixed height so the field stays put.
  `ProgramCatalog::listed` is false until the host has scanned once ("Loading programs…").
- the sessions (`root/sessions.rs`): every non-desktop session in the account's settings, opened in
  a floating window, and New session.
- "Show this display's bars" while one is hidden.
- Menu (`desktop.apps.menu`), be-host's menu.

## Window chips

`WindowChips` (`desktop.windows`, `root/windows.rs`) is built from the dock's tabs and their
children's statuses: displays, then windows, then placeholders, then the block windows the shell
opened, in tab order. The focused chip is drawn pressed and a hidden one hollow; a click shows and
focuses a window (`Workspace::show`), and a click on the focused one hides it (`Workspace::hide`).
Each chip's context menu offers Hide or Show, Close and, with several displays, "Move to display N"
(`<chip>.display.<n>`). A placeholder's chip offers Open and Dismiss, and a Restore chip
(`desktop.windows.restore`) opens every placeholder. `display_of` lists a window on the display
holding most of it.

Along a horizontal bar the chips share the band's length like a taskbar's: they split it evenly up
to `CHIP_WIDTH`, switch all to icons below `CHIP_LEAST`, then scroll inside their own `Scroll` (so
the grip stays put), revealing the focused chip. An empty row shows "No windows"
(`desktop.windows.none`) a chip wide.

## Window keys

The window manager is beui-dock-host's (guides/beui/docking.md, the dock's window actions), acting on
program and block windows alike as floating dock windows. Under them, display 0's dock is one
`desktop` pane (`hide_empty_bar`, so empty it draws nothing): a tab dropped in its middle fills it
and one dropped at an edge splits it, tiling the desktop beneath the windows. Each dock of the desktop sits in its own
`DockHost`; `DeskWindows` passes every display, and its work area, to `DockHostLayout::set_areas`.
`root/window_keys.rs` binds the dock's `window_actions` at `KeyAt::Window`, capturing, so they work
over programs; they are disabled while `ScreenLockedWatch` is set. workspace-ui binds none.

| Action | Linux | Where the system keeps Super |
| --- | --- | --- |
| Close window | Alt+F4 | Ctrl+F4 |
| Snap left or right half | Super+Left/Right | Ctrl+Alt+Left/Right |
| Maximize | Super+Up | Ctrl+Alt+Up |
| Restore a snapped window, else hide it | Super+Down | Ctrl+Alt+Down |
| Show desktop | Super+D | Ctrl+Alt+D |
| Move to next or previous display | Super+Shift+Right/Left | Ctrl+Alt+Shift+Right/Left |
| The program's next window | Super+` | Ctrl+Alt+` |
| Full screen | Super+F | Ctrl+Alt+F |
| Window switcher | Alt+Tab, Alt+Shift+Tab | the same |
| Leave the switcher | Alt+Escape | the same |

The system keeps Super where the host answers `SuperKeptWatch` true (Windows, whose shell takes
Win+arrows and where Alt+F4 closes be-host itself). A key with nothing to do (Super+Up on a
maximized window) is not taken, so it reaches the program. A Super chord is not a Super tap. To
drive the switcher, hold Alt across the presses: `drive keydown alt; drive key tab tab; drive keyup
alt`. Super+drag moves a floating window and Super+right-drag resizes it.
