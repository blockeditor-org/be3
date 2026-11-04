# The Wayland compositor

Under `--desktop` on Linux be-host is a Wayland compositor, `crates/be-host-wayland` (built on
smithay), with Xwayland for X11 programs (guides/desktop/linux.md). This guide is how a program's
windows, input and data move between the compositor and be-host. What the shell does with the
windows is guides/desktop.md.

## Running programs

The socket is `wayland-<n>` in `XDG_RUNTIME_DIR`, or in a private directory when that is unset (as
under `:dev`; `runtime_dir.rs`). Programs started by the compositor get `Compositor::environment`:
`XDG_RUNTIME_DIR`, `WAYLAND_DISPLAY`, `DISPLAY` for Xwayland and `DCONF_PROFILE`
(`gtk_settings.rs`, which hides GTK's window buttons). In a login session `DISPLAY` and
`WAYLAND_DISPLAY` also go to D-Bus's activation environment and systemd's user manager
(be-host's `src/login_session/environment.rs`).

The Apps panel lists the `.desktop` programs of the XDG data dirs (be-host-wayland's `programs.rs`
reads them and their icons; `programs::load_icon` reads PNG, SVG and XPM) and runs one, or a typed
command, through `programs::request(ProgramAction)`. Against a driven be-host:

    drive exec foot            # runs foot inside it
    eval "$(drive env)"        # the same variables in a shell, for wl-paste and the like

`weston-simple-shm` (package `weston`) and `foot` are small clients to try. `kill -STOP` on a
client freezes it, to see "not responding"; `kill -CONT` brings it back.

## Windows

A toplevel is a `be_host_wayland::WindowView`: a forwarding catcher the shell places like any
other node (guides/beui_keyboard.md, One input path). `Windows` (`windows.rs`) is the window list;
`WindowInfo` what is known of each. X11 toplevels are the same: `ClientWindow` holds a smithay
`desktop::Window` that is either an xdg toplevel or an `X11Surface`, so everything below applies to
both unless it says otherwise.

**Size.** A window is configured to the rect its view is placed at, always, with the tiled states.
The dock decides sizes; there is no negotiation. Until first placed it is configured 0×0 so it
picks its own size: `WindowInfo::size` is the size it last chose itself (a commit not answering a
size the compositor asked for), which becomes the window child's `intrinsic`. `WindowInfo` also
carries the min and max size (`set_min_size`/`set_max_size`, X11 `WM_NORMAL_HINTS`). A 1×1 buffer
(Firefox commits one for server-side-decorated windows) opens at a default size. Maximize is not
offered in `wm_capabilities`, since the dock owns the size, but a request is answered with the
maximized state.

**Drawing.** `render.rs` draws a window's last buffer top-left at its own size, clipped to the view,
never stretched, until the client commits the new size
(`a_resized_window_is_drawn_unscaled_until_it_commits_the_new_size`). A self-committing surface
that changes only its viewport keeps its old view (`held_view.rs`).

**Lifecycle.**

- A toplevel that commits with no buffer leaves the list until it maps again
  (`ServerEvent::Unmapped`).
- A window placed nowhere (its tab hidden: placed hidden, which mounts no view) is told it is
  suspended: xdg `suspended` (`xdg_wm_base` v6) or `_NET_WM_STATE_HIDDEN` (`Windows::suspended`).
- Minimizing (`set_minimized`, X11 iconify) counts up `Windows::minimized`; `show_window_menu`
  becomes the child's `menu-asked`; `xdg_system_bell_v1` counts up `Windows::rung`.
- `xdg_activation_v1` (`activation.rs`) bumps the window's `raised` through `Windows::activate`;
  programs the compositor starts get a token as `XDG_ACTIVATION_TOKEN` and `DESKTOP_STARTUP_ID`.
- The compositor pings a client when its window is clicked, typed into, focused or asked to close;
  no answer within `PING_TIMEOUT` (`state.rs`) is "not responding". Closing such a window kills
  the process if the compositor launched it, and disconnects the client. X11 windows have no ping.

**Focus.** The catcher's focus is the window's keyboard focus (`Windows::focus`); a view's `focus`
counter gives its window the keyboard each time it counts up. A view rebuilt elsewhere (into or out
of fullscreen) takes the focus back if the one it replaces had it
(`Windows::unmounted_focused`). For X11 `focus::KeyboardFocus` also sets the X input focus.

**Parents and dialogs.** `xdg_toplevel.set_parent`, an X11 `WM_TRANSIENT_FOR`, and `xdg-foreign`
v2 (a portal's file chooser) give a window a parent; `State::parent` reports the nearest shown
ancestor. `xdg-dialog-v1` (`dialog.rs`) or `_NET_WM_STATE_MODAL` sets `WindowInfo::modal`, and
`Windows::blocked_by` sends the parent's keyboard to its innermost modal dialog and drops the
parent's pointer input.

**Popups** (xdg-popup, X11 override-redirect windows) are fitted to the monitor their window is
mostly on and drawn in a floating overlay above the document, so a menu can pass its window's edge.
A grabbing popup holds the keyboard until dismissed; on destroy the keyboard returns to the parent
before the client's next request is answered (`XdgShellHandler::grab` explains why). An X11
override-redirect window is a layer of the toplevel it belongs to (`WM_TRANSIENT_FOR`, else the
window under the pointer or with the keyboard).

**Moving and resizing from the program.** `xdg_toplevel.move`/`resize` (X11
`_NET_WM_MOVERESIZE`) are honoured only while the press they name is still held
(`ServerEvent::Grabbed`); the view hands them to its `Carrier`, which be-host gives the views of a
floating window (`be_host_desktop::carrier`) and which begins the same gesture as the window's bar.
A docked window has no carrier and does not move.

**Icons and names.** `xdg-toplevel-icon-v1` gives the tab's icon (pixels shrunk to 64, or a theme
name). Without one, be-host-desktop's `linux.rs` uses the desktop entry the window was launched
from, else the one its app id or `WM_CLASS` names (`Environment::entry_of_window`, also matching
`StartupWMClass`). An X11 `_NET_WM_ICON` is not read (smithay does not expose it).

**Kept across restarts.** `host-window.origin` is the `.desktop` entry and `launch` number when the
compositor launched the program or an ancestor, else the executable from `/proc` (X11:
`_NET_WM_PID`). be-sdk-shell reopens a placeholder from that (guides/desktop.md, Program windows).

**Other protocols.** `ext_foreign_toplevel_list_v1` (`toplevel_list.rs`) lists windows to any
client. `wp-content-type-v1` and `wp-tearing-control-v1` are stored but change nothing yet.
`wp_alpha_modifier_v1` multiplies a surface's alpha. `wp-single-pixel-buffer-v1` buffers draw as
one-pixel textures. `ext-session-lock-v1` is not implemented.

## Input

beui decides which window a press, motion, scroll or key goes to; `Compositor::after` turns what
each `WindowView` was forwarded into `wl_pointer` and `wl_keyboard` events. Where the shell draws
over a window it takes nothing (`occluders`), and a press stays with the window it began on.
Buttons beyond beui's five are `PointerButton::Other` with their evdev code
(`drive down TARGET button:0x115`).

- **Scrolling** (`State::pointer_axis`): a wheel goes out as `wl_pointer` source `wheel` with
  `axis_value120` and libinput's 15 per notch, so clients scroll their own wheel step. A slide
  between `ScrollStarted` and `ScrollEnded` is source `finger` and ends with `axis_stop`, which
  clients fling from; any other slide is `continuous`, which GTK reads as a trackpoint and Firefox
  scrolls by as wheel clicks (a tenth of its value each).
- **Pen** (`tablet.rs`; beui's `Event::Pen` and `Input::Touchpad` are in guides/beui/platforms.md, Input): a `zwp_tablet_tool_v2` of one tablet made on first use, with pressure and
  tilt. A client that never asked for a tablet seat gets the pen as its pointer.
- **Touch** (`touch.rs`): each finger goes to the surface it went down on, as `wl_touch`; a client
  with no `wl_touch` gets its first finger as the pointer. The seat offers touch only while a
  touchscreen is attached or a finger has been seen, because GTK, SDL and browsers change their
  interfaces when they see the capability.
- **Pointer constraints** (`pointer.rs`): a lock or confinement holds only while its surface has
  the pointer and its window the keyboard. A lock sets `pointer_locked` on the view, so the platform
  hides and holds the cursor (guides/beui/components.md, holding the pointer) and raw motion arrives as relative motion.
  `zwp_pointer_gestures_v1` carries touchpad gestures.
- **Shortcuts inhibitor** (`shortcuts_inhibit.rs`): while its window has the keyboard, the view
  sets `inhibits_shortcuts` and beui keeps window listeners from it (guides/beui_keyboard.md).
  Super+L always locks, and Super+Escape (`register_restore_shortcuts`) takes the shortcuts back and
  releases a held pointer until the window is pressed again.
- **Window drags** (`wl_data_device.start_drag`) go to the window beui finds under the pointer.
  Only a shown window takes a drop, and the drag icon is not drawn (BE3-296). Files dragged in from
  the platform (`drive grab FILE`, `drive drop`) become the compositor's own drag
  (`selection/files.rs`) offering `text/uri-list`.

## Keyboard layouts

Settings > Input > Layout takes several layouts ("us,de"), compiled into one xkb keymap with a
group each, for programs (`State::set_keyboard`) and under DRM for the seat (beui-platform-drm's
`Keyboard`). be-host's `input/linux.rs` keeps the active group and publishes
`KeyboardLayoutsWatch` (labels from `named_layouts`, using xkb's `rules/evdev.xml`). The desktop's
`switch-layout` locks a group on the xkb state (`Compositor::set_layout`), which reaches programs as
`wl_keyboard.modifiers`' `group`. In a window be3's own text keeps the outer desktop's layout, so
only programs switch. A group an xkb option switches (`grp:alt_shift_toggle`) is not followed by the
indicator. A keymap that does not compile keeps the last and toasts.

## The on-screen keyboard

beui's on-screen keyboard (guides/beui/space.md, The on-screen keyboard) is the desktop's. In a login
session it opens when a field is tapped with no keyboard plugged in; the Quick settings tile and the
Status bar button open it regardless. The host draws it along the bottom of the focused field's
window and lays that window out above it.

Programs reach it through `zwp_text_input_v3` (`text_input.rs`; there is no input-method protocol,
since the keyboard is the compositor's own). While a focused window's text input is enabled, its
`WindowView` is a text field whose `keyboard` asks count enables, so re-enabling (GTK, on a tap)
asks again. Its keys have no physical place, so `layout.rs` types each with the key that makes it in
the active layout, at whatever Shift/AltGr/level-5 level, with modifiers just around the press: a
German y comes from the US z key. Shortcuts with a letter no key makes (Ctrl+C on Russian), and named
keys, go from their US place. Text no key types (é on US, emoji) is sent as `commit_string`, so a
program without an enabled text input never gets it. X11 programs and programs like foot use no
text input, so the keyboard never opens by itself over them: use the bar's button
(`desktop.keyboard`), which keeps the focus where it was.

## Cursors

In a login session the DRM platform draws the pointer into each output's frame from the XCursor
theme (`cursors.rs`, CSS names then X11 names). Over a window, a `wp_cursor_shape_v1` shape becomes
its `CursorIcon` (`Windows::cursor`); a `wl_pointer.set_cursor` surface reaches the DRM platform as
a `beui_platform_drm::SoftwareCursor` texture (be-host-desktop's `desktop_host/linux.rs`), and other
platforms as `cursor_image` (`Windows::cursor_image`, shm buffers only; winit shows it, headless
reports it in `drive state`). Under `BEUI_LAYERS=space` the layer draws its own cursor from the
`CursorIcon`.

## Frame pacing

In a login session a frame runs only when an output has flipped (beui-platform-drm's `pacing.rs`),
and `wl_surface.frame` callbacks and `wp_presentation_feedback` are answered by the flip of the
display the window mostly covers (`beui_platform_drm::Flips` to `Compositor::presented`,
`presentation.rs`). Elsewhere they are answered when the window is painted; when nothing presents
(headless, or an occluded window) a shown window that drew is answered on the next frame anyway, so
GTK, which draws only after its last frame was answered, keeps working when driven headless.

`wp_fifo_v1` and `wp_commit_timing_v1` (`pacing.rs`) pace commits against the same presentation. A
fifo barrier clears when the window is presented, or at once while it is not shown, so a FIFO
swapchain never stalls behind a hidden window.

## X11 programs

`Server::start_xwayland` starts `Xwayland` on the first free display with smithay's `X11Wm`
(`xwayland.rs`; decisions/xwayland_starts_with_the_compositor.md). It needs `Xwayland` on `PATH`
(`apt-get install xwayland`) and a writable `/tmp/.X11-unix`; failures are logged as `be-wayland:
Xwayland did not start ...`. Each X11 window is configured at its place on the desktop
(`State::place`), so X coordinates are the desktop's. X11 `WM_STATE` stays `NormalState`, since
smithay writes `IconicState` only by unmapping the frame. To try it, the log names the display:

    DISPLAY=:0 xterm
    DISPLAY=:0 GDK_BACKEND=x11 xfce4-terminal --disable-server

## Clipboard and the primary selection

be-host's clipboard and the Wayland clipboard are one (`selection.rs`, `compositor/clipboard.rs`).

- A program that sets the selection (through `wl_data_device` or `ext-data-control-v1` /
  `zwlr-data-control-v1`, which `wl-copy` and clipboard managers use) is asked for its text, read
  without blocking and handed to be-host (`Context::copy_text`); the program keeps its selection
  with all its types. A selection that offers only images is not handed over.
- Otherwise the selection is `HostClipboard`, read from the platform only when a program pastes.
  A platform cannot say when its clipboard changes, so a program's selection gives way to the
  host's when be-host's window is focused again and the platform's text is no longer the program's.
- X11 shares both through `X11Wm`'s selection callbacks (`State::announce_x11_selection`,
  `State::x11_paste`, `Owner::X11`). Wayland sees an X11 selection's text targets and targets whose
  names hold a `/`. smithay's `X11Wm::new_selection` does not flush its connection, so an overlay
  (`third-party/rust/overlays/smithay/xwm.rs`) does.
- Middle-click paste is off unless the input settings turn it on; while off, the primary selection
  stays empty (`selection/primary.rs`).

To try: `wl-copy TEXT` and `wl-paste` (`wl-clipboard`), `xclip` with `DISPLAY` from `drive env`,
and `drive key ctrl+c`/`ctrl+v` in the host. `drive clipboard TEXT` is a copy in the outer desktop;
between `drive blur` and `drive focus` it also takes the selection from a program.
