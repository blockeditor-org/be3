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
(`WindowHosting::Any`, so every `Window` is a real window), rewrites the
platform's input onto the panels, composites them into the one main window it
leaves shown, and maps points between that window and a panel (`up`, `down`).
Its tests call `Space::input` and `Space::frame` with a `LayerView` directly.

## Where the code is

beui-layer-space: `space.rs` is the layer (requests, focus, input routing,
the cursor, composition); `panel.rs` a window's surface in the world and its
placement string; `head.rs` the head, views and the body; `carry.rs`,
`contact.rs`, `resize.rs` and `chrome.rs` moving and reshaping panels;
`laser.rs` controllers; `scale.rs` raster density; `layers.rs` headset
composition layers; `render.rs` and `space.wgsl` the GPU compositor;
`picture.rs` the scene as a painting for tests.

beui-layer-xr: `host.rs` (`XrSpace`, the layer wrapping `Space`), `link.rs`
(finding a headset, opening the GPU), `runtime.rs` (loading the OpenXR
runtime), `headset.rs` (session, frames, swapchains, layers), `controllers.rs`,
and `unsupported.rs` for every platform but Linux and Windows.

## Panels and windows

Each window is a `Panel`. The main window's panel opens at the platform
window's size, filling 80% of the view's width, and never closes. A window's size in
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

**Grab.** While its window has focus space holds the pointer locked. Losing
focus or holding Escape lets it go; the next press only grabs it again and
reaches nothing.

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
W, A, S and D walk, Q and E roll. Nothing pressed while steering reaches the
program, so a program in space never sees a Ctrl+Shift chord; nor do
Ctrl+Alt+M and Ctrl+Alt+arrows (`tuning.rs`).

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
`Keyboard` panel: corner brackets to scale, sides to resize, the window's title
in a pill above to carry, and buttons below to move, rotate, roll and bend. The
title is the `Window` component's `title`; its `notice` (which the dock sets for
a tab that wants attention) shows in the title's place. Input on chrome never
reaches the program.

## Drawing

Space draws through a list of `head::View`s. The simulator has one, the head over the platform window; an XR
host hands `Space::set_views` the tracked eyes each frame, and space then moves
its head to their centre so the cursor, carry and chrome work off the centre
eye. `Space::draw` draws the scene into each view's target texture (an XR
swapchain image, say); the platform window gets the mirror through the
`Drawing` that `Space::frame` returns. A frame is composed again only when the
head, a panel, the cursor or a window's output changed.

Each window's display list is painted by its own `beui_renderer_wgpu::Renderer`
into a mipmapped texture, only when its output changed and only its damage.
The floor is part of the backdrop, with only the grid lines in 3D, so a panel
below the floor stays visible and pickable.

**Sharpness.** Each panel is rastered at the most pixels per point any view
shows it at (`scale::wanted`), in steps, held until the change is big enough
and has lasted 250 ms, and frozen during a gesture. A change reaches the
program as `WindowReport::Rastered`: it repaints at the new raster without
laying out, so text only gets sharper or softer. The raster goes through to
apps: an `Embed` hears it (`EmbedPlacement::raster`), be-host sends an app
region its raster beside its scale, and be-sdk-beui-platform reports it to the
region's window as `Rastered`, so app content is as sharp as the window without
relayout.

**Cursor.** While space holds the pointer, or a program locks it, space hides
the platform's cursor and draws its own in the scene (`cursor::Sprite`, with
`beui_core::drawn_cursor::paint`), showing the icon the window under it asks
for. Once the grab lets go, the platform draws the cursor.

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
- **Losing it.** `headset::Lost`: a lost session reopens on the same device, a
  lost instance starts the search again, and an ended session leaves the
  simulator for the rest of the run.
- **Frames.** The headset paces them: `Layer::pace` blocks in `xrWaitFrame` and
  its predicted display time becomes `LayerView::now` and the frame clock. While
  a layer paces, the runner turns the window's vsync off and keeps framing even
  while the window is minimised. Each eye has its own swapchain and pass.
- **Composition layers.** Panels whose order to the eye is plain are also
  handed to the runtime as quad or cylinder layers (`Space::layers`,
  `layers::choose`), so its compositor samples them once and text stays sharp;
  space draws a transparent hole for each in the eye buffer, so nearer panels,
  chrome, beams and the cursor still cover it.
- **Mouse and keyboard** work as in the simulator through the mirror window;
  steering moves the body (`head::Body`) instead of the head, since the headset
  owns the head. Controllers come from an OpenXR action set and enter as
  `Incoming::Controller` (see Lasers).

## Testing space and headsets

- beui-layer-space's tests (`src/tests/`) drive `Space` directly with a
  `LayerView` and compare paintings of the scene (`picture.rs`, which orders
  panels by depth as the GPU does). Behaviour tests check what reaches the
  program and what space reports.
- `drive --into=space ...` and `drive xr HAND ...` drive space and the lasers
  (guides/running/drive.md); placement through a real `Runner` is tested by
  `InSpace` in crates/beui-host/src/tests.rs.
- beui-layer-xr's tests run Monado's service with its null compositor on
  lavapipe (`//buck/sysroot:amd64-xr`) and check frames are shown, including a
  headset that appears after the `Link` started and a service restart. They
  share one `XDG_RUNTIME_DIR`, so each holds the `MONADO` lock.

## The on-screen keyboard

beui has an on-screen keyboard of its own, modelled on Android's Unexpected
Keyboard (a key types its middle when tapped and its corners when swiped
toward them), for a screen with no keyboard attached and for typing with a
pointer or a laser.

It is three pieces:
- `beui::native::keyboard::unstyled` (crate `beui-keyboard`): the layouts as
  Rust data (`layout.rs`; there is no user configuration), the Fn and script
  tables, the compose table (`compose.rs`, generated from X.org's
  `en_US.UTF-8/Compose`), and `Engine`, the pure gesture and modifier state
  machine its tests drive. All keys sit in one forwarding `Interactive` with
  `press_focus=false`, so each finger reaches the engine as its own pointer and
  a press does not move the focus.
- `beui::native::keyboard::styled::SoftwareKeyboard`: the themed keys, the bar
  with the close button, and the emoji panel. `install_keyboard` puts a
  `KeyboardWindow` on a document as its `KeyboardHost`; `beui_host::native::build`
  does it for every document.
- `beui_core::software_keyboard`: whether it shows and where.

**Where it shows.** A context shows it only after `Context::enable_keyboard`,
which the runner calls unless `Platform::shows_keyboard` says the platform has
its own (Android, the web, an app's host). It is a document of its own, shown
after the program. Normally `Document::show` takes its height
(`Context::set_keyboard_share`) off the bottom of the window the focused field
is in, lays that window's content out above it, and paints the keyboard there
(`Context::draw_into`). Where windows open (`Context::opens_windows`, as in
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
for something that types nothing, except in space, where the person placed it.

**Typing.** The engine makes `Input::Key`s with real modifier presses and no
physical key (`evdev` is `None`, built by `beui_core::input::typing`, which
`drive type` uses too, so a Wayland program gets the key that makes each
character in its own layout; guides/desktop/linux.md), and `Input::Text` for
what no key types. The runner feeds them back as the next frame's input
(`FrameOutput::typed`), so a beui field, an app and a Wayland program all take
them as from a real keyboard. On the HTML core the keyboard shows only
where a view places it, keys are dispatched as `KeyboardEvent`s, and
`toggle_keyboard` shows the browser's own (`navigator.virtualKeyboard`). The
Inputs page of beui-demo shows the keyboard in place.
