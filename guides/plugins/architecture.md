# Plugin architecture

How plugins work underneath: the framework crates, the plugin contract, the host's compositor and
runtime, frame pacing, accessibility, fonts, the GPU path, crashes, what a wasm plugin cannot do,
building, and extending the protocol. Read it when changing the plugin system or when an editor
needs something the API in `guides/plugins/editor_api.md` does not give it.

## The shape of it

- The plugin system is events in and pixels out: the host sends the input it routed, and the
  plugin hands back the surface it drew. beui is only what the framework puts in between.
- A plugin is one wasm module for every platform. It runs beside the app, not inside it: a thread
  of its own under wasmtime, a worker of its own in the browser.
- A plugin has no connection, block store, key, network or file system of its own. The app is the
  only peer, and everything goes through the host.
- Neither side polls the other. Everything a plugin says arrives on its own, in order, and wakes
  the host; everything the host says reaches the plugin the same way.

## Framework crates

| Crate | Role |
|---|---|
| `crates/block-plugin-api` | the protocol: messages, manifest, host values; rules in `PROTOCOL.md` |
| `crates/block-editor-plugin` | the core, framework-free: speaks the protocol, keeps each instance's `EditorHost`, gives every screen the host asks for a surface of its own, owns the wgpu device behind them |
| `crates/beui-adapter-plugin` | beui's adapter for a plugin region |
| `crates/block-editor-beui` | what every beui editor goes through; re-exports the core, so an editor depends on it alone |
| `crates/block-ui` | anything host and plugin both draw (labels, shared painting helpers) |
| `crates/block-ui-test` | the test harness (`guides/testing.md`) |
| `crates/block-plugin-sdk` | the shared module plugins link against (`guides/plugins/sdk.md`) |

### The core: `Plugin` and `Instance`

block-editor-plugin hands each instance to a `block_editor_plugin::Plugin`, whose `Instance`:

1. Is told the lifecycle: connect, creation, artifacts.
2. Is given the protocol's own `InputEvent`s for a region.
3. Is asked for a `Frame` per region:
   - whether anything changed and when to be drawn again;
   - the rectangles the host is told about;
   - its `WindowRequests` (below);
   - `claims`, presses the host should give the plugin ahead of its children;
   - `description`, its accessibility tree ([Accessibility](#accessibility));
   - `stopped` ([Crashes](#crashes)).
4. Is asked to paint the regions that changed, each into a `PaintTarget`: the device, the queue,
   the view and format of that region's surface, its size, the region, and its age.

Age:

- The `Region` an update is given carries the same age, so an instance can settle what it will
  repaint before it paints.
- At age 0 the core has cleared the surface and the instance paints all of it.
- Otherwise the texture still holds what the instance painted that many presents ago, so it need
  only repaint what changed since.
- `paint` returns the rectangles it drew into; the core reports them with the frame, so the host
  redraws only those.

### Window requests

`WindowRequests` are sent to the host as `EditorMessage::Window` for that region, only when they
change:

| Request | Meaning |
|---|---|
| cursor | shown by the host while the pointer is over that screen |
| `pointer_locked` | hold the pointer still and hide it; the editor then reads `PointerMotion` events instead of a position. Held while any screen of any instance asks |
| IME area | its keyboard is `Keyboard::Held` or `Keyboard::Shown { asks }`, where `asks` counts the taps that should bring the on-screen keyboard back |
| back | whether its document has a back handler enabled; the host sends back to the last screen it drew that has one, unless its own document handles back first |
| `wants_keyboard` | a layer that traps focus is open; the host gives it the keyboard until it clears |
| `intercepted_keys` | chords of its intercepting actions (below) |
| paste kinds | the paste kinds it takes |

Intercepted keys: the host offers those chords ahead of whatever is focused and sends each press
it takes, its repeats and its release to that screen as `InputEvent::InterceptedKey`, after the
`Modifiers` they were made with, then keeps sending `Modifiers` until the chord's modifiers are
let go. A tap chord (an action on `Chord::tap`) is sent as `InputEvent::InterceptedTap` while the
screen does not have the keyboard. block-app does this for the shell's frame only.

### The beui adapter

beui-adapter-plugin is a beui_core `Adapter` and `Platform`, like the platform adapters. Its
`PluginSurface`:

- runs one `App` through beui's `Runner`;
- turns the host's `InputEvent`s into beui's (including `PointerMotion` and `Focus`);
- answers copy, paste, file picks, the cursor, the IME and the back gesture through the
  `EditorHost`;
- paints with beui's wgpu renderer into the region's surface, keeping the damage of earlier
  presents so it repaints only what the texture's age needs.

### block-editor-beui

- Implements `Instance` with a `PluginSurface` per region (`BeuiPlugin<A>` for a `BeuiApp`).
- Runs a beui `Context` per screen the instance is given, builds `BeuiApp::view(editor)` once,
  keeps the `Document` it produced, shows it in its region, and paints the frame straight into
  that screen's surface, which the host composites.
- Adds the editor's own pieces: `Editor`, the frame's chrome and menu, children.
- Describes the frame's menu to the host as `MenuEntry` rows (`EditorMessage::Menu`) and runs the
  row the host sends back (`MenuPick`).
- Colours are written the way the surface wants them: the plugin surface is not an sRGB format,
  so beui encodes gamma there, and linear where it draws its own window.

### Monitors

The host tells each region which monitors it covers (`ViewportMetrics::monitors`, each an id, a
name and a rect in the region's points, from the host document's screens;
`block_editor_plugin::Region::monitors` in the region's surface). beui-adapter-plugin makes those
the document's screens (`guides/beui/components.md`, screens), so a plugin spanning several
monitors, like linux-desktop, centres its dialogs on one and shows its volume level on each.
`set_monitors` on block_ui_test's harness gives a test the same.

## The host

### Compositor

- Each region an instance is shown in is a retained beui component, `PluginRegion` in
  `crates/block-app/src/compositor/region.rs`. It mounts the screen for as long as it is built,
  places it from the `Embed` it fills, draws the screen's pieces, and gets its input forwarded by
  beui (`Interactive`'s `on_forward` and `forward_at`, which leaves the holes its live children sit
  in to them). It claims the presses a plugin reported with `claim_at`.
- `crates/block-app/src/compositor/editors.rs` builds tabs, frame takeovers, embedded children and
  previews out of it. What a plugin sends reaches them as one revision signal per plugin, so
  nothing on the host walks the editors each frame.

### Discovery

- The build stages a `<plugin id>.plugin.json` per plugin and `plugins.json`, the list of them,
  beside the modules: beside the executable natively, in the APK's assets on Android, at the
  bundle root in the browser.
- Native and Android share one loader (`crates/block-app/src/editors/plugin/discovery/bundle.rs`)
  and differ only in how a file is read; the browser fetches the same files (`discovery/web.rs`).

### Runtime and backends

- Every platform runs the same host: `plugin_host/runtime.rs` owns the runtimes, the instances
  they show, input, errors and restarting.
- A platform only implements its `Backend`: starting a runtime, exchanging messages with it, and
  handing over the frame it drew.
  - `plugin_host/wasm.rs` runs the module under wasmtime everywhere but the browser.
  - `plugin_host/web.rs` runs the same module in a worker in the browser.
- `plugin_host/instances.rs` routes messages to and from instances.
- `plugin_host/host_values.rs` holds the host value permission tables
  (`guides/plugins/editor_api.md`, Host values and actions).

### Graph commands

The graph calls on `Blocks` become `GraphCommand`s the session sends as `CreateBlock`,
`SetParent` or `Define`. The host applies each to its mirror at once, checks the account may edit
what it touches, and hands it to the app's peer. The peer belongs to the app, so a block an
instance created still reaches the server after the instance closes.

## Frame pacing

A plugin runtime draws only when the host asks it for a frame; it paces nothing itself.

- When the host tells it something that changes what it shows, or something wakes it, it says a
  frame is needed and waits. A burst of input it cannot keep up with is drawn as one frame
  carrying all of it.
- Its last frame says how long it wants to wait before being drawn again. The host holds that
  until it comes due and answers with one request in its own frame, so an editor animating
  without pause is drawn once per host frame, and one not on screen is asked for nothing.
- The request leaves as soon as the frame's input has been delivered (`plugin_host::start_frames`,
  run from beui's `Document::on_interacted`), before the host lays anything out, so the editor
  draws while the host lays out. It carries the frame's input, routed against where the editor
  sat and what covered it last time it was drawn. Input the host routes always brings a request.
- After layout the host sends each runtime the size and place its regions were given, asks for
  another frame where that changed what it shows, and waits for the answers before it paints
  (`plugin_host::settle`, run from `Document::on_laid_out`), up to `FRAME_BUDGET` in
  `plugin_host/runtime.rs`, then lays out again whatever the answers moved.
- A host child placed by one plugin's answer learns its size only from that layout, so the hook
  runs again after any layout that moved something, and a nested editor is asked in the same
  frame within the same budget.
- An editor that answers in time is shown in the frame that asked. One that runs over is shown in
  a later frame without holding the host longer; until then its last painting is drawn at the
  size it was painted, cropped to the region or short of it, never stretched.
- The browser cannot block on a worker, so there the answer always lands in a later frame.
- A message that changes nothing drawn (an acknowledgement) costs no frame on either side. A frame
  runs only the instances a message or wake touched, those that asked to be drawn again, and all
  of them when the layout moves, so an idle instance beside a busy one costs nothing.
- Frame takeovers are the one exception to following a resize at once: see
  `guides/plugins/pan_and_zoom.md`, Taking the frame over.

## Accessibility

While a screen reader is on (the platform asked the host for its accessibility tree) or a driver
is attached (`guides/running/drive.md`), the host sends `Message::Describe(true)`.

- Every `Frame` region's `FrameReport` then carries a `Description`: an AccessKit `TreeUpdate` of
  the editor's tree, plus the actions it registered (`guides/beui/input.md`).
- The tree is in the region's coordinates (points from its top-left) and uses the editor's own
  node ids.
- It is incremental: the first after a `Region::describe` change is the whole tree, later ones
  only the nodes that changed, and `None` when nothing did. `Region::describe` is `None` while
  nobody listens and changes whenever the host needs the whole tree again (describing turns on,
  or the region gets a new screen). block-editor-plugin keeps the trees of unreported frames, so
  none is lost.
- The host keeps a copy of each region's tree (beui's `accessibility::Embedded`), renumbers its
  nodes into its own, cuts it into a valid tree whatever the plugin sent, and shows it as a
  `Pane` named after the plugin under its window, with the editor's focus while the region has
  the keyboard.
- beui-adapter-plugin fills it from the document's own tree, which carries the test ids, so a
  beui editor needs nothing beyond its labels and `@test_id`s. An editor on another framework
  fills `Frame::description` with AccessKit itself.
- The host sends back `Message::NodeAction` (the screen reader's `ActionRequest` in the editor's
  node ids), giving the region the keyboard for an action that moves focus into it (focus, click,
  setting a value), and `Message::RunAction` to run one of the editor's actions.
  beui-adapter-plugin carries out both.

## Fonts

- A plugin carries no fonts and may open no font file. The host sends the fonts it uses
  (`Message::Fonts`) as soon as the session is running, and `block_editor_beui::fonts()` is the
  one `beui::FontLibrary` every context of the plugin is built over.
- A character none of them covers is reported back (`Message::MissingCharacters`). A native host
  answers with a system font that covers it (fontconfig on Linux, the system font directories
  elsewhere); the library adds it and every region lays its text out again. The web host has no
  system fonts to offer.
- Tests never see a system font: block_ui_test's harness uses `beui::FontLibrary::bundled()`, so
  snapshots paint the same on every machine.

## The GPU path

- A plugin never learns where its pixels live. It asks for a render target and is handed an
  opaque texture.
- Under wasmtime that is a texture the app already owns in its own device: nothing is exported or
  imported.
- In the browser it is a texture on a device the plugin's worker owns. The worker draws each place
  the host shows the plugin into a canvas the page transferred to it, and the page keeps those
  canvases behind the app, which leaves a transparent hole wherever one shows through.
- The plugin's wgpu calls reach the device over the ABI in `crates/block-gpu-abi`, whose
  functions `gpu_functions!` lists once for every side:
  - the guest side is block-gpu-guest, against wgpu's custom backend;
  - the host side is `block_gpu_host::calls`, which wasmtime links directly and the browser
    exports from `crates/block-gpu-shim`, a wasm-bindgen module the worker instantiates first,
    binds the plugin's imports to, and hands the plugin's memory to read pointers from.
- Only the plugin's first thread may talk to the GPU; a call from any other is refused by name.
- An editor drawing with the GPU rather than beui's shapes uses a beui `Drawing` node painting a
  gpu `Drawing` through `draw_gpu` (`guides/beui/components.md`, gpu drawing). The gpu work sits
  behind a `Draw` the renderer runs while it paints, so a headless test, which builds no renderer,
  never opens a device. A snapshot never holds what it drew, so its test asserts on the block and
  what the editor paints around it.

## Crashes

- `Frame::stopped` says a region will draw nothing more: `Stopped::Closed` when its app closed
  itself, `Stopped::Failed` with the panic. The core sends it once as `EditorMessage::Stopped`; a
  beui region says it when its `Runner` exits. The host shows the error with a restart in the
  region's place.
- A plugin that panics stops alone. block-editor-plugin's panic hook hands the message and its
  place to the host through `be3_host.host_panic` (a guest's stderr does not reach the host). The
  runtime keeps it as the plugin's error, every region of it shows `beui::styled::CrashNotice`
  with a Reload button that restarts the runtime, and the host reports a toast
  (`notices::report`).
- A panic in the host's own code while it works for one plugin stops that plugin the same way,
  not the app: inside `Runtime::contain`, which `poll`, `settle`, `with` and the other entry
  points in `runtime.rs` go through, and on the plugin's worker thread.

## What a wasm plugin cannot have

A plugin is one wasm for every platform (`wasm32-wasip1-threads`), so a dependency it cannot have
there it cannot have at all:

- nothing that reaches the network (use `fetch` through the host);
- nothing that opens a file (ship data with `editor(data = ...)`, read through the host);
- nothing wasm-bindgen.

Other constraints:

- `std::thread` works: the host implements thread spawning against the plugin's shared memory.
  Only the first thread may use the GPU.
- A C or C++ library links in as a static library for `wasm32-wasip1-threads`, named in
  `editor(extra_deps = [...])`, and must be compiled position-independent to link against the SDK.
  The PDF editor links PDFium, an Emscripten build that `third-party/pdfium` adapts to WASI (a
  shim for what Emscripten's JavaScript provided, and one thread calling it, since Emscripten
  built it without atomics); it is not position-independent, so the PDF plugin is built whole
  with `shared_sdk = False` (`guides/plugins/sdk.md`).
- C that calls `setjmp` (FreeType, which beui shapes and rasterizes glyphs with) is compiled with
  the SjLj lowering the WASI sysroot's setjmp needs, which turns those calls into wasm
  exception-handling instructions. The host runs plugins with that proposal on, but a module
  carrying a tag section loads only in a runtime with exception handling;
  `./scripts/buck run //crates/block-wasm-host:example-instantiate` says whether one does.

## Building and staging

- Every crate under `crates/editors` is staged as a plugin. `./scripts/buck run
  //crates/block-app:native` builds each for `wasm32-wasip1-threads` with the plugin profile (its
  own configuration, because the app links wgpu with real backends while a guest links only the
  custom one).
- Each is laid out beside the app with its manifest renamed `<plugin id>.plugin.json`, and
  compiled to `foo.cwasm`, which opening the editor maps in rather than waiting seconds for
  Cranelift. The app compiles `foo.wasm` itself whenever the `.cwasm` is missing or was made by a
  different wasmtime, which is how a cross-compiled build runs.
- The entry point names a file beside the manifest, where the build already put it, so nothing is
  moved to be found.
- `//crates/block-app:plugins` is the plugins alone. `guides/buck2.md` has the rest.

## Extending the protocol

When an editor needs something the host has and the plugin does not:

1. Add it to `crates/block-plugin-api`: a `Message` or `EditorMessage` variant, `validate`
   coverage, the block ids it carries in `block_ids.rs` (so a checkout's scope applies to them),
   and a round-trip test.
2. Accept it in the plugin's `ClientSession` state machine.
3. Route it in `crates/block-app/src/plugin_host/instances.rs`.
4. Surface it on `EditorHost` or `App` (and on `Editor` for beui editors).
5. Describe any new rule in `crates/block-plugin-api/PROTOCOL.md`.

A message a plugin sends reaches the host by itself unless it is one of the session's own (the
handshake, acknowledgements, errors, ping and shutdown), which `Message::is_session` names.

For a value or action only the host can see or do, add a host value or action instead
(`guides/plugins/editor_api.md`, Adding one); that needs no new message.
