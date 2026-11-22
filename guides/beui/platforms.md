# beui platforms

What runs underneath a beui program: the runner every platform shares, how
input is normalized, the clipboard, the share sheet, renderers, and what is particular to each
platform. How a program uses them is in guides/beui/programs.md.

## Where the code is

- crates/beui-runner: `Platform`, `PlatformServices`, `Runner`, `Layer`, the
  input intake (`intake.rs`), the `Renderer` trait and `environment`
  (`BEUI_PLATFORM`, `BEUI_RENDERER`, `BEUI_LAYERS`).
- crates/beui-host-window: picks a native platform (`native_platform`), loads
  the renderer and stacks layers (`src/runner.rs`); crates/beui-host does the
  same for the web (`web_platform`).
- crates/beui-platform-winit (desktop), beui-platform-web (+ beui-web-input,
  the browser's event mapping), beui-platform-android (with its Java in
  `android/`), beui-platform-drm (displays and libinput with no display
  server), beui-platform-headless (no window: tests and `:dev`).
- crates/beui-renderer-wgpu: the one renderer.

## The runner

A platform owns the event loop and translates native input into `Incoming`s
for `Runner::push`. Everything a frame does is `Runner::frame`: it applies
window reports, normalizes input, runs the program, publishes accessibility,
and tells the platform's `PlatformServices` what the program asked of the
window. `Frame::wait` is how long the platform may sleep (`None` until woken);
a platform frames only when an event arrived, something asked for a repaint,
or a `Waker` was woken.

**Window state.** What the program wants of its window is one
`beui_core::window::WindowState`, filled by the `Context` during the frame:
`requests` (`WindowRequests`, the same type an app sends its host: cursor,
pointer lock, back handling, keyboard wanted, intercepted keys, paste kinds,
IME area), `cursor_image`, `typed` and `appearance`. The runner calls
`show_window` only when it changed. One-off actions (copy, share, paste, file pick,
fullscreen, close) have calls of their own. `typed` counts frames that
delivered text, because typing moves the platform's own copy of a field (the
web's hidden textarea, Android's editable) even when the program rejects it;
those platforms resync their copy when the program's text changes.

**Reports.** The window reports `resized` (physical pixels), `scaled` and
`fullscreen` through `Runner::reports()`, a `WindowReports` a platform may
clone and keep. A window drawn at another raster (a DRM display's, an app
region's) reports `WindowReport::Rastered`. The runner applies reports at the
start of the next frame, resizing renderers and setting the context's pixels
per point before layout, so platforms never resize a renderer or pass a scale
themselves. Fullscreen goes both ways: the program asks with
`Context::set_fullscreen`, and reads what the window did with
`Context::fullscreen`.

**Cursors.** Under touch emulation the cursor is `CursorIcon::Touch`, a finger
circle defined in `beui_core::touch_cursor`. A platform with a cursor of its own
shows the system's; where beui draws the cursor (DRM, mouse simulation, space) it
draws `beui_core::drawn_cursor::paint`'s icons
(decisions/our_own_cursors_where_we_draw_them.md). A custom
`cursor_image` from the hovered `Interactive` wins over its icon; only winit
shows it, and it is not in the app protocol.

**Fitting a window to its content.** `<Window fit={Some(max)}>` takes
min(content, max): `Document::refit_windows` measures the content before
layout. An app view placed at its content's size is such a window
(crates/be-sdk-api/PROTOCOL.md, `child-placement.fit`).

**Accessibility** is published only while something listens
(`Runner::accessibility_requested`), and a document redescribes only the nodes
a change reached. Each layout space (a scroll's content, a canvas) is a
container node with the space's transform, so scrolling changes one node.

**Closing and panics.** After the frame in which the program calls
`close_window`, `frame` returns `None` with `Runner::exited` set and the
platform stops (Android finishes its activity, the web closes the tab where
allowed, an app's frame tells its host it stopped). A window closed from outside
calls `Runner::os_close`; the web asks on `beforeunload`, where a refusal is
the browser's leave-the-page prompt. A panic in the program stops the runner
rather than unwinding through the event loop: the program hears `exiting`,
and `Runner::finish` returns the panic as an error after releasing the screen.
A program that can survive a panic in one part catches it there.

## Unseen windows

A window nobody can see is `Runner::set_occluded`: its frames still run
(layout, paint, accessibility, a driver), but each one's output goes to
`Renderer::note`, which keeps what it damaged, and nothing is presented. The
headless platform is always occluded and draws only for a driver's `shot`.

`Context::seen` tells the program whether the current frame will be drawn. A
shot makes an occluded window seen until it is taken, in the first frame whose
`Program::drawing` says nothing is still to draw. be-host passes `seen` to its
apps in each `DrawFrame`: an unseen app runs its frames without encoding them
(except previews, which others show as pictures), and when the host is seen
again it asks each such app for a full frame, which `drawing` waits for.

A window with no surface (minimised to nothing, or an Android surface between
`surfaceDestroyed` and the next) runs no frames, unless a layer paces them. The
runner drops the input that arrives meanwhile but keeps what ends something (a
lift or cancel, a key or button release, focus, the input method's events, a
cancelled back gesture) for the next frame that runs; a back gesture invoked
then is dropped, since it would go back from a page nobody saw. What it keeps
does not count as `has_events`, so a platform does not spin waiting.

## Input

A platform pushes an `Incoming`: an `Input` in physical pixels, logical pixels
or points (`Units`), a file drag (`Incoming::FileDrag`, which never crosses the
app protocol: the app's host turns it into `drag-over`), a timestamp, or a
window report. The intake is the one place input becomes `Event`s, after the
program has taken what it routes itself (`Program::input`, offered each input
in points just before it becomes events, with the timestamp the platform gave
it, if any); drive's
gestures, the on-screen keyboard and typed pastes push `Input` there too, so
they are normalized like a window's. At the start of each frame it converts to
points, stamps keys and buttons with the held modifiers, drops a held
modifier's repeats and control characters, turns the primary button into a
finger under touch emulation, holds `PointerGone` back while a button is held,
and releases modifiers when the keyboard is lost (`Input::Focus(false)` means
only the keyboard). Keys and `Keystroke` are guides/beui_keyboard.md.

It queues with `beui_types::input::enqueue`, the same rules an app's host
merges batches by: scrolls and zooms fold into the last of a run, and each
pointer is heard once per frame at its latest move, but an event between two
moves keeps both, so a click lands where it was made. An `Interactive` with
`every_sample` (the canvas pen during a stroke) sets
`WindowRequests::every_sample`, and then every move is kept and `on_drag` runs
per sample.

Each pointer is named by a `PointerId`: `Mouse`, each `Touch` finger (an XR controller's
laser is a touch, `is_controller`), `Pen`, and two the platforms never report: `Keyboard`, a
keyboard carrying a dragged item, and `Relayed`, a carry whose points another program or
window relays (a dock bar an app's grip moves across the app protocol).

Per-device behaviour that spans platforms:

- **Timestamps** (`Incoming::Time`): the web, Android and DRM use the device's;
  winit stamps on arrival.
- **Prediction.** The web pushes `Input::Predicted` after a pressed move
  (`getPredictedEvents`), carried as `PointerPress::predicted`. The next sample
  replaces it, so draw it only as a transient tail; never store it.
- **Pinch** is an `Event::Zoom` everywhere: winit's `PinchGesture`, a
  Ctrl-wheel in Chrome and Firefox (`PINCH_SPEED` per pixel), Safari's
  `gesturechange`. The web prevents their default so the page never zooms.
- **Touchpad gestures** (`Input::Touchpad`: swipe, pinch, hold) come from
  libinput and winit (`touchpad.rs`; winit has no swipes) and go only to the
  forwarding node under the pointer (a Wayland window in be-host-wayland).
- **Trackpad scrolling.** `Input::ScrollStarted` and `ScrollEnded` bracket
  fingers on the pad; between them a `Scroll` follows the fingers and may
  stretch, and the lift flings. A wheel with no start (a mouse) clamps at the
  ends. winit maps `MouseWheel` phases and on macOS drops the system's momentum
  (`wheel.rs`, [decision](../../decisions/beui_winit_drops_macos_scroll_momentum.md)).
  DRM brackets libinput's finger scrolls, which end with a zero. The browser
  reports no phases, so web wheel scrolls never stretch or fling.
- **Wheel notches.** A platform reports a mouse wheel in `ScrollDelta::Lines`,
  each `SCROLL_LINE` points; `Event::Scroll` and `PointerPhase::Scroll` keep the
  count as `notches`, so a forwarding node can hand on a wheel as a wheel.
- **Cancelling.** `Input::PointersCancelled` ends every press, touch and pen
  without a release, and cancels a pointer grab. winit and the web send it on
  losing focus; winit on Wayland also when the pointer leaves with a button
  held, because a compositor grab sends `wl_pointer.leave` and no release.

## Clipboard and paste

A copy is a `Clip` (`beui-types/src/clip.rs`): representations of the same
content, richest first, each a format id and its data, or a `Render` that makes
the data only when a paste asks for it. Format ids and the be3 formats (serde
types encoded with postcard: `TextFragment`, `Blocks`, `CanvasItems`, `Files`)
are in `be-lib-ui-types/src/clipboard.rs`; standard ones keep their MIME type
(`text/plain`, `text/html`, `image/png`). An item names blocks only in its
`refs`, beside its bytes, which index them by position (a format's
`ClipFormat::refs` and `set_refs`; its ids are `#[serde(skip)]`, as a `TextFragment`'s embeds are), so whoever
carries a clip translates them without decoding it: be-host-runtime does for an
instance in a checkout on every path a clip crosses (a copy, a render on
request, a drag, a paste into a screen). `reactive::copy(Clip)` or
`Context::copy` writes; `Context::watch_copies` hears every copy, and
`Context::read_clipboard` reads a format for the program itself;
be-host-wayland bridges the Wayland clipboard with these two.

A platform keeps the clip it last copied (`HeldClip`) and answers pastes from it
while it is still the clipboard's, so lazy representations and be3 formats
survive where the system clipboard cannot carry them. Windows knows by the
clipboard's sequence number and carries be3 formats as registered formats; the
web writes them as `web ` types where `ClipboardItem.supports` says so
(Chromium) and reads them from the async clipboard; X11 offers them as targets
of its selection and reads them from another program's; macOS knows by the
pasteboard's change count and writes them as pasteboard types, reverse-DNS UTIs
made from their format id (`org.be3.clip.blocks`); Android lists them among
the clip's description's MIME types and keeps them, base64, in its extras, with
a nonce of the copy that tells its clip from another's. A clip crosses Binder,
whose transactions stop near 1 MB, so text and extras share a budget of UTF-16
units: the text has first claim, beyond which it goes there cut short with a
notice (`clip::cut_short`), and formats that would overrun what it leaves are
left out (`clip::fitting`); both stay whole in the held clip, which answers
pastes in the program until the clipboard changes even when nothing could be
put there
(`beui-platform-android/src/clipboard.rs`, written without an emulator: only
its compiling is checked). All of them hold an item's
`ClipItem::stored` bytes, refs and all. Elsewhere the plain text on the
clipboard is compared with the text written, as beui's HTML core does too
(`page::copy` and `page::held`): a `paste` event on a node answers from the
held clip when its text matches. On the system clipboard a platform puts the
plain text and the PNG together where it can: X11 (and Wayland, through
XWayland) by owning the CLIPBOARD selection from a thread of its own
(`beui-platform-winit/src/clipboard/x11.rs`, INCR for large data, handing the
clip to a clipboard manager with SAVE_TARGETS when the program closes), Windows
as `CF_DIB` and a registered `PNG` beside the text, macOS as `public.png` beside
it; elsewhere the plain text, or a PNG when there is none. Its Xvfb tests run
where `Xvfb` is installed and skip with a message where it is not, as on the
build workers. The macOS code has no Mac to run on: only its compiling is
checked, and the rule it shares with Windows (`beside`) is tested.

Text inside beui has `\n` line breaks. Every read of plain text from a system
clipboard (each platform, the HTML core's `paste` event, be-host-wayland's
reads of a program's selection) goes through `clip::text_from_system`, which
turns `\r\n` and a lone `\r` into `\n`; on Windows a copy writes `\r\n`
(`clip::text_with_crlf`). `HeldClip::wrote` compares line breaks as `\n` on
both sides.

The focus reads the clipboard only by pasting: an `Interactive`'s `pastes`
lists the format ids it takes, preferred first, and its `on_paste` gets a
`ClipItem`. `request_paste` (a Paste menu) or Ctrl+V asks
`PlatformServices::paste` for the first of them the clipboard holds; Ctrl+Shift+V
asks for plain text only. The answer enters the intake as `Input::Paste`, after
earlier input; `Input::PastePiece`s before it, which the app protocol sends for
data over its message limit, are joined into it. Platforms that already deliver
Ctrl+V as a paste (`delivers_paste_shortcut`: the browser, an app's host) are not
asked again.

An `Interactive` with `clipboard_keys` (a terminal) takes Ctrl+C, V, X and A as
plain keys. This matters only on the web, which decides in `keydown` before the
program sees the key: it stops the browser's copy, paste and cut but lets
Ctrl+Shift+V make a `paste` event. A node that takes these keys and leaves
Ctrl+V unanswered gets no paste on the web.

## Share sheet

`reactive::share(Clip)` hands a clip to the platform's share sheet, and
`reactive::use_share_sheet()` says whether there is one: show a Share control
only while it is true. The native core keeps the clip until the end of the frame
(`Document::share`, `Context::share`, `FrameOutput::shared`), when the runner
calls `PlatformServices::share`, where `has_share_sheet`, read each frame, says
there is a sheet. Call it from the press: the web shares only within a user
gesture, which the end of the press's frame still is. The web
(`beui-web-input/src/share.rs`, used by both cores) has one where
`navigator.share` is defined, and passes the clip's plain text as `url` when it
is one link (`clip::shared_link`), else as `text`; Android always has one, an
`ACTION_SEND` of the plain text through the system's chooser (`BeuiView.share`,
only compile-checked). Desktops and DRM have none, nor does beui inside an app
yet: the app protocol does not carry a share. The headless `Simulation` has one
when a test sets `share_sheet`, and keeps what was shared in `shared`.

## Renderers

Platforms name no renderer crate: they draw through a
`beui_runner::renderer::Renderer` that beui-host's loader returns, attached to
a raw window handle (`WindowSurface`) or made with its element
(`CanvasSurface`). The wgpu renderer splits into `Resources`, shared by every
target on one device, and a `Renderer` per target; give every window or panel
on a device a renderer over the same `Resources`, as be-sdk-beui-platform does
per app module, beui-layer-space for its panels and beui-platform-drm for its
displays. `Renderer::capture` reads the presented frame back for a screenshot. A renderer
`set_window` marks (present.rs's, each DRM display's) is composited while the program turns
`beui_draw::compose` on, as be-host does: its UI goes into root textures (one more above each
anchored view), with surfaces
drawings show in place, and the window is drawn from its layer list (guides/apps/runtime.md,
The layer list); `Renderer::scissors` is then where that pass drew.

## Per platform

**Desktop (winit).** Retitles the window, shows custom cursor images, reads
clipboard images through arboard. `setup.get::<Arc<winit::window::Window>>()`
is the window. A resize is answered with a frame at the new size at once, not at
the next redraw. Until it shows, the system fills what the window grew by: on
X11 with the clear colour (`backdrop.rs` sets the window's background), on
Windows black or white, which winit cannot change (plans/deferred/winit_issues.md).
On Windows wgpu draws through Vulkan alone, which beui-layer-xr and
be-host-win32-capture are written against (plans/deferred/windows_vulkan_and_d3d12.md).

**Web.** `run_web(element_id, options, layers, program)` draws with WebGPU, or
WebGL where there is none, into the element if it is a canvas or a canvas it
adds. Keys go through a hidden textarea, so paste and composition behave like
any input on the page. A press on the canvas, or a text field taking beui's
focus any other way, focuses that textarea; the on-screen keyboard opens only
within a user gesture. Pointer events push every one of
`getCoalescedEvents()`. The page's thread may never wait: a browser traps
`Atomics.wait` there, so a page module starts with `be_wasm_threads::initialize_page`
and anything that waits runs in a worker (guides/running/web.md, "Files and offline").
guides/running/web.md runs it.

**Android.** The activity extends `com.be3.beui.BeuiActivity`, whose
`BeuiView` is the surface and takes touches, keys, the soft keyboard's input
connection and AccessKit. It is a view so that TalkBack's touch exploration
reaches it as hover events, which NativeActivity's input queue never delivers
([decision](../../decisions/beui_android_uses_its_own_view.md)). The library
defines `#[unsafe(no_mangle)] fn android_main(program: AndroidApp)`, called on
a thread of its own, which calls `run_with`. It draws edge to edge: the program
fills the safe area, and the bars show its clear colour. guides/running/android.md
builds it.

Android's surface comes and goes with the screen (locking, switching apps). The
renderer is wrapped in `beui_runner::renderer::Leased`: `surfaceDestroyed`
revokes the surface's `SurfaceLease`, which waits only for a draw in progress,
never for a frame, and the renderer touches that surface no more. `BeuiView`
is a `SurfaceHolder.Callback2`, and runs `surfaceRedrawNeededAsync`'s
`drawingFinished` once a frame is presented on the new surface, so Android never
shows it blank or stale. `surfaceDestroyed` first waits up to 100 ms for the
runner to detach, so the swapchain goes before the surface (as EGL wants)
unless a long frame is running. Before API 26 Android
waits on `surfaceRedrawNeeded` on its UI thread, for at most a second.

**Soft keyboards** take their room out of the window: on Android the inset
joins the safe area, following each frame of the keyboard's slide
(`WindowInsetsAnimation`), and a keyboard up when the window lost focus (the
phone locked) is shown again when it regains it; on the web the viewport asks for
`interactive-widget=resizes-content`. When the shown rectangle shrinks while
the focus takes text, the document scrolls the focused field into what is
left. beui's own on-screen keyboard is guides/beui/space.md.

**Headless.** `beui_platform_headless::Simulation` is the window: a
`PlatformServices` with a clipboard, a share sheet a test can turn on, and a screen to go fullscreen into, and the
only window a driver can `resize`. beui's tests drive one with a renderer that
draws nothing.

## Displays under beui-platform-drm

`Drm` (offered through `RunOptions::platforms`, picked with
`BEUI_PLATFORM=drm`) drives the displays and input devices itself; the Linux
login session runs on it (guides/desktop/login_session.md). It shows no main window
and opens only windows it placed (`WindowHosting::Placed`), one per display.
The program gets a `beui_platform_drm::Displays` handle through `Setup`, opens
a `Window` at `displays.placement(id)` and the display's size for each
display, and the platform matches it to the display in the same frame, so it
lays out at the right size in the frame that first shows it. It reports each
window's `Resized`, `Moved` and `Scaled`, but no placement, so a dock never
reopens one; any other `Window` is an overlay. A display with no window shows
the clear colour. The main window is the first display's size and scale.

`DisplayConfig` arranges the displays: which are on the desktop, mirroring or
off (`uses`), which is first (`main`), where each sits (`places`) and their
scales (see its doc comment, `plan.rs` and `layout.rs`). `Monitor`s list every
connected display, including those off or mirroring. Pointer input goes to the
window of the display under the pointer, in points, and a press focuses that
window. `Displays::screenshot` reads every desktop display's last drawn picture
back (`Screen::capture`) and joins them into one (`screenshot.rs`). Tests make a `Displays` of their own with `Displays::new` and change
it with `set`.

**Screen input** (`screen_input.rs`, a `ScreenInput` handle through `Setup`): the program sets
walls and regions (`ScreenInput::set`) and hears what happened at each (`state`, `watch`); it
holds no policy of its own. A `Wall`, a stretch of a display's side, stops relative pointer motion
even where another display begins, and says whether the pointer rests `against` it and how far
motion that began there ran on past it (`pushed`, which only grows). A `Region` says whether the
pointer is in it and counts fingers landing in it; one that `captures_touch` takes a finger landing
in it until it lifts (`touch`, `lifts`, `lifted`), and the finger never reaches a window, not
even once the region goes while it is down. Every
finger landing anywhere is counted too (`touch_downs`). Absolute pointers (tablets) and a locked
pointer pass every wall. The logic is plain (`Input`), tested in `screen_input/tests/`.
