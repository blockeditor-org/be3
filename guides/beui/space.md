# Space, headsets and the on-screen keyboard

Space (`crates/beui-layer-space`) is a beui `Layer` that shows a program's
windows as panels in a first-person 3D scene. It is both the VR simulator,
which runs on any wgpu adapter with a mouse and keyboard, and the scene an
OpenXR headset shows through `crates/beui-layer-xr`. `BEUI_LAYERS=space` stacks
the simulator over any native or headless (`:dev`) run; `BEUI_LAYERS=xr` uses a
headset instead and falls back to the simulator while there is none. Layers in
general, `LayerView` and `RunOptions` are in [programs](programs.md); windows,
`WindowRequest` and `WindowReport` in [docking](docking.md). What remains to
build is in plans/vr.md.

No platform knows about space, and all its 3D math (`glam`) stays in its crate.
It works entirely through the `Layer` hooks: it opens every window itself
(`WindowHosting::Any`, so `Context::opens_windows` is true and every `Window` is
a real window), rewrites the platform's input onto the panels in `input`,
composites the panels into the one main window it leaves shown, asks for frames
while it animates, and maps points between the platform window and a panel
(`up` through the ray at a point, `down` by projecting a panel's point). Its
inherent `Space::input` and `Space::frame` take a `LayerView` directly, which
its tests call.

## Where the code is

beui-layer-space, by file:
- `space.rs`: `Space`, the layer. Window requests, focus, input routing, the
  cursor, pointers and their held state, carry and resize bookkeeping,
  composition.
- `panel.rs`: `Panel`, a window's surface in the world (pose, size,
  metres per point, bend), ray hits, and the placement string.
- `head.rs`: the head, views and fields of view, the body a headset's tracking
  space is placed by.
- `carry.rs`, `contact.rs`, `resize.rs`: moving a panel, keeping it out of
  others, resizing.
- `chrome.rs`: the controls drawn around each panel and their gestures.
- `laser.rs`: controller rays. `hold.rs`: holding Escape. `tuning.rs`: the
  temporary cursor-up setting.
- `scale.rs`: how many pixels per point each panel is rastered at.
- `layers.rs`: which panels a headset shows as composition layers.
- `render.rs`, `space.wgsl`, `floor.rs`, `cursor.rs`, `notice.rs`: the GPU
  compositor, the backdrop and grid, the drawn cursor, the waiting notice.
- `picture.rs`: the scene as a painting, for tests.

beui-layer-xr: `host.rs` (`XrSpace`, the layer wrapping `Space`), `link.rs`
(finding a headset, opening the GPU), `runtime.rs` (loading the OpenXR
runtime), `headset.rs` (session, frames, swapchains, layers), `gpu.rs` (Vulkan
through `XR_KHR_vulkan_enable2`), `controllers.rs`, `unsupported.rs` (every
platform but Linux and Windows).

## Panels and windows

Each window is a `Panel`, keyed by `WindowId`. The main window's panel opens on
the first frame at the platform window's size in points, at eye height, far
enough to fill 80% of the view's width; it never closes. A window's size in
space is its logical size and is reported as `WindowReport::Resized`; the
platform window keeps its own size and stays the mirror of the scene.

`WindowRequest::Open` placements:
- `Host`: a pose space wrote earlier in `WindowReport::Placed`
  (`Panel::placement`, the string `space X Y Z QX QY QZ QW S B`). The dock keeps
  these in memory and hands them back when it reopens a window; nothing keeps
  them across restarts.
- `Over { window, at }`: on another panel's point, a little in front of it.
- `AtPointer`: on the cursor's ray, facing the viewer. The dock floats a tab
  dropped on nothing this way.

Roles: a `Keyboard` window ignores its placement and opens under the focused
field; a `Popup` opens where placed. Neither ever takes the focus, and a press
anywhere but on a popup closes it and goes no further. Popups and overlays have
no chrome and take no part in contact.

Focus follows clicks, not the cursor, as on a desktop; opening or carrying a
panel also focuses it. It is reported as `WindowReport::Focused`, which is how
the program's document lets go of and restores the keyboard focus inside each
window. A press on no panel focuses none, and then keys reach no window.
Panels have no stacking order, only depth: a `Raise` of a hidden panel flashes
its outline instead of moving it.

## Input

**Grab.** While its window has focus space holds the pointer locked, so the
mouse sends only relative motion. Losing focus, the platform dropping the lock
(a browser on Escape) or holding Escape for 0.6 s lets it go; the next press
into the window only grabs again and reaches nothing.

**The cursor** is a point in the world, not on the screen. Presses, moves and
hovers go where the ray from the eye through the cursor meets the nearest
panel, in that window's points (on a bent panel, the curved surface's point); a
press's own position is ignored. Turning the head leaves the cursor where it
is, so it can be anywhere around the viewer; a marker at the screen's edge
points to it when it is out of view. An absolute position (a platform
`PointerMoved`, or a driver's point in a window) puts the cursor where its ray
hits. Moving onto another panel just names the new window, so a drag goes on
across windows; off every panel with a button held, events go on to the
window the press began in, at points outside its rectangle, which is how a
program tells a drop on nothing.

**Steering.** Ctrl and Shift held together steer: mouse motion turns the head,
W, A, S and D walk along the floor, Q and E roll. Nothing pressed while
steering reaches the program, so a program in space never sees a Ctrl+Shift
chord. Ctrl+Alt+M and Ctrl+Alt+arrows change a temporary setting for which
"up" the cursor swings about (`tuning.rs`); those keys don't reach the program
either. Time comes from `LayerView::now`, the frame clock.

**Pointers.** The mouse and each controller's laser are separate pointers
(`space::Pointer`), each with its own held state (`space::Held`), so both hands
and the mouse can each carry, resize or press at once. A program's `BeginMove`
or `BeginResize` goes to the laser whose touch is down in that window, else to
the mouse.

### Lasers

Controllers reach space as `Incoming::Controller` (`beui_runner::Controller`):
a pose per hand (aim and grip, in the views' world with the floor at y = 0),
trigger and squeeze from 0 to 1, `Lost`, and `Toward { window, at }`, which
drivers use to aim at a window's point. An XR host sends them every frame;
`drive xr` sends them in tests (guides/running/drive.md).

A laser acts as a finger, not a hovering mouse: the trigger sends
`Input::Touch` where its ray hits, so beui's touch handling applies (a drag
scrolls, a long press opens the context menu, two lasers on one panel pinch).
A trigger press on chrome works it as a mouse press does. Squeezing the grip on
a panel carries it along the laser; squeezing both grips on one panel moves,
turns and scales it with the two hands. While any laser has a ray, space tells
beui no keyboard is attached (`Layer::keyboard_attached`), so tapping a field
opens the on-screen keyboard as a panel.

## Moving and reshaping panels

A program asks to move or resize with `WindowRequest::BeginMove` and
`BeginResize` (a title bar's drag, the dock's tab drag). Space starts one only
while a press it gave the program is still held, as Wayland ties a move to its
press; a request that arrives after the release is dropped
([decision](../../decisions/beui_space_move_needs_a_held_press.md)). Letting go
ends it: the program gets the release, and space reports the new placement as
`WindowReport::Placed`. The program sees no pointer moves meanwhile.

A carried panel keeps its pose relative to the head, turns to face the viewer,
and moves nearer or farther with the scroll wheel (which the program does not
get). It does not pass through other panels: it stops 12 mm short, keeping the
side it was on, until it is pushed through deliberately (`Carry::settle`).
Resizing sets the size from the dragged edge and reports `Resized` each frame,
so the program lays out at the new size at once.

Chrome (`chrome.rs`) is drawn and handled by space around every `Normal` and
`Keyboard` panel: corner brackets (drag to scale the panel, keeping its points),
sides (drag to resize), the window's title or notice in a pill above (drag to
carry), and buttons below to move, rotate, roll and bend it. The title is the
window's `WindowState` title and the notice is the `Window` component's
`notice`, which the dock sets for a tab that wants attention. Input on chrome
never reaches the program, and chrome is ignored while the program holds a
button. Chrome keeps one apparent size to the eye.

## Drawing

Space draws through a list of `head::View`s (pose, off-axis field of view,
target size). The simulator has one, the head over the platform window; an XR
host hands `Space::set_views` the tracked eyes each frame, and space then moves
its head to their centre so the cursor, carry and chrome work off the centre
eye. `Space::draw` draws the scene into each view's target texture (an XR
swapchain image, say); the platform window gets the mirror through the
`Drawing` that `Space::frame` returns. A frame is composed again only when the
head, a panel, the cursor or a window's output changed.

Each window's display list is painted by its own `beui_renderer_wgpu::Renderer`
into a mipmapped texture, only when its output changed and only its damage.
The floor is part of the backdrop, coloured by each pixel's direction, with
only the grid lines in 3D; a panel below the floor stays visible and pickable.
Panels are depth tested, chrome after them; laser beams after the chrome; the
outlines of a push's ghost and a raise's flash after those with no depth test;
the cursor last, over everything.

**Sharpness.** Each panel is rastered at the most pixels per point any view
shows it at (`scale::wanted`), in steps, held until the change is big enough
and has lasted 250 ms, and frozen during a gesture. A change reaches the
program as `WindowReport::Rastered`: it repaints at the new raster without
laying out, so text only gets sharper or softer. The raster goes through to
apps: an `Embed` hears it (`EmbedPlacement::raster`), be-host sends an app
region its raster beside its scale, and be-sdk-beui-platform reports it to the
region's window as `Rastered`, so app content is as sharp as the window without
relayout.

**Cursor.** While space holds the pointer, or a program locks it, space asks
the platform for no cursor and draws its own in the scene
(`cursor::Sprite`, painted with `beui_core::drawn_cursor::paint` under a
`WindowId` of its own), sized to cover as many pixels as it has points at any
depth. It shows the icon the window under it asks for, or a resize or grab
icon over chrome. Once the grab lets go, the platform draws the cursor.

## Headsets

`XrSpace` (beui-layer-xr) wraps `Space` as the layer. It builds on Linux and
Windows; elsewhere the `Link` fails at once and space stays the simulator.

- **Finding a headset.** `beui::window` makes a `Link`, named by
  `RunOptions::program_id` (else the program's name) for the runtime's
  dashboard. It retries every 2 s on its own thread, so a runtime or headset
  started later is found without a restart, and wakes the loop on each status
  change. Without a headset, space is the simulator with a "Waiting for a
  headset" notice over the mirror.
- **One GPU device.** The OpenXR runtime picks the GPU, so the `Link` is the
  renderers' `OpenGpu` and opens the device through `XR_KHR_vulkan_enable2`;
  the window, the panels, dmabufs and the headset share that one device.
  `OpenDevice::extensions` carries the Vulkan device extensions be-host needs
  (be-host-dmabuf's `DEVICE_EXTENSIONS` on Linux, be-host-win32-capture's on
  Windows). When a headset turns up after the device was opened, the device is
  stale: the renderer reopens it as after a lost GPU (`Renderer::recover`,
  `Gpu::reopen`), the runner hands the new device to the program
  (`Program::renderer_replaced`) and the layers, and `XrSpace::setup` takes the
  `Headset`.
- **Losing it.** `headset::Lost` says what happens: a lost session reopens on
  the same device, a lost instance (the runtime quit) starts the search again,
  and an ended session (the runtime closed the program) leaves the simulator for
  the rest of the run.
- **Frames.** The headset paces them: `Layer::pace` blocks in `xrWaitFrame` and
  its predicted display time becomes `LayerView::now` and the frame clock. While
  a layer paces, the runner turns the window's vsync off and keeps framing even
  while the window is minimised. Each eye has its own swapchain and pass.
- **Composition layers.** Panels whose order to the eye is plain are also
  handed to the runtime as quad or cylinder layers (`Space::layers`,
  `layers::choose`), so its compositor samples them once and text stays sharp;
  space draws a transparent hole for each in the eye buffer, so nearer panels,
  chrome, beams and the cursor still cover it.
- **Mouse and keyboard** work as in the simulator: the mirror window keeps the
  keyboard focus and the pointer, and the cursor's screen is the mirror (the
  centre eye at the simulator's field of view). Steering moves the body
  (`head::Body`) instead of the head, since the headset owns the head.
- **Controllers** come from an OpenXR action set (`controllers.rs`) and enter as
  `Incoming::Controller`, as described under Lasers.

## Testing space and headsets

- beui-layer-space's tests (`src/tests/`) drive `Space` directly with a
  `LayerView` and compare paintings of the scene (`picture.rs`, which orders
  panels by depth as the GPU does). Behaviour tests check what reaches the
  program and what space reports.
- `drive` works through space: `drive --into=space ...` sends input below it,
  `drive xr HAND ...` works the lasers (guides/running/drive.md). Placement
  through a real `Runner` is tested in crates/beui-host/src/tests.rs (`InSpace`).
- beui-layer-xr's tests run Monado's service with its null compositor on
  lavapipe (`//buck/sysroot:amd64-xr`) and check frames are shown, including a
  headset that appears after the `Link` started and a service restart. They
  share one `XDG_RUNTIME_DIR`, so each holds the `MONADO` lock.

## The on-screen keyboard

beui has an on-screen keyboard of its own, modelled on Android's Unexpected
Keyboard, for a screen with no keyboard attached (a login session on a tablet)
and for typing with a pointer or a laser. Each key types its middle when tapped
and its corners when dragged toward them; a few keys take straight swipes too
(arrows on Compose, a caret slider on Space, word selection on Backspace).
Modifiers latch for one key on a tap and lock on a long press. Compose follows
X11's compose sequences and previews on each key what it would type next.

It is three pieces:
- `beui::native::keyboard::unstyled` (crate `beui-keyboard`): the layouts as
  Rust data (`layout.rs`; there is no user configuration), the Fn and script
  tables, the compose table (`compose.rs`, generated from X.org's
  `en_US.UTF-8/Compose`), and `Engine`, the pure gesture and modifier state
  machine its tests drive. The `SoftwareKeyboard` component puts all keys inside
  one forwarding `Interactive` with `press_focus=false`, so every finger reaches
  the engine as its own pointer and pressing a key does not move the focus.
- `beui::native::keyboard::styled::SoftwareKeyboard`: the themed keys, the bar
  with the close button, and the emoji panel. `install_keyboard` puts a
  `KeyboardWindow` on a document as its `KeyboardHost`; `beui_host::native::build`
  does it for every document.
- `beui_core::software_keyboard`: whether it shows and where.

**Where it shows.** A context shows it only after `Context::enable_keyboard`,
which the runner calls unless `Platform::shows_keyboard` says the platform has
its own (Android, the web, an app's host). It is a document of its own, shown
after the program. Normally `Document::show` takes its height off the bottom of
the window the focused field is in, lays that window's content out above it,
and paints the keyboard into that window (`Context::draw_into`). The height is
a share of the window (`Context::set_keyboard_share`, which be-host sets from
the display settings). Where windows open (`Context::opens_windows`, as in
space) it takes nothing off the screen and shows in a `Window` with
`WindowRole::Keyboard` instead; window reports go only to the document holding
the window, so the keyboard's document hears its own `Rastered`.

**Opening and closing.** `toggle_keyboard()` is the one way to ask for an
on-screen keyboard, whichever shows it: beui's where enabled, else the platform's
(Android's keyboard, the web's hidden textarea, or an app's host through
`WindowRequests::keyboard_toggles`). The desktop's Quick settings tile, the
inspector's Sim tab and the mouse simulation's button call it. While the
platform reports no keyboard attached (`WindowReports::keyboard_attached`: only
`beui-platform-drm` sends it, from libinput devices with an A key, and a layer
such as space may override it), the keyboard also opens when the focused field
asks (`ImeArea::keyboard`, guides/beui_keyboard.md) and closes when focus leaves
for something that types nothing. In its own window (only in space) it stays
open until closed, since the person placed it.

**Typing.** The engine makes `Input::Key`s with real modifier presses and no
physical key (`evdev` is `None`, built by `beui_core::input::typing`, which
`drive type` uses too, so a Wayland program gets the key that makes each
character in its own layout; guides/desktop/linux.md), and `Input::Text` for
what no key types. They go out in `FrameOutput::typed` and the runner feeds them
back as the next frame's input, so a beui field, an app and a Wayland program
all take them as from a real keyboard. On the HTML core the keyboard shows only
where a view places it, keys are dispatched as `KeyboardEvent`s, and
`toggle_keyboard` shows the browser's own (`navigator.virtualKeyboard`). The
Inputs page of beui-demo shows the keyboard in place.
