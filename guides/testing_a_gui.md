# Testing a GUI

GUI tests run headless: no window, no input devices, no server. A test builds an editor,
drives it as a person would, and checks what the block became and what the editor painted.
They are fast enough to run in `./scripts/verify`.

There are four kinds, by what they host:

- an editor alone, against a fake host (sections 2 to 5): be-sdk-test's `BeuiTest`;
- a beui document that is not an editor, such as a beui-demo page: be-sdk-test's
  `DocumentTest` (4, "Documents that are not editors");
- be-host itself with real apps in it, for what happens between editors (6):
  crates/be-host-tests;
- a screen that must behave the same on beui's native and HTML cores (7): crates/beui-tests.

## 1. What a GUI test looks at

- **The block.** An editor turns gestures into operations, so assert on the content those
  operations produced.
- **The painting**, as a snapshot (4): one frame or a recording of several. It catches what an
  assertion on the block cannot: a control that vanished, a panel that lost its contents, a
  colour that changed.

Don't assert on coordinates, widget sizes or the order of the accessibility tree: they change
whenever anyone touches a layout and say nothing about whether the editor works
(decisions/gui_tests_check_the_block_and_the_painting.md).

## 2. Give the widgets test ids

Tests name widgets by a test id, not by their label, so renaming a button or moving it to a
sidebar leaves the tests alone:

    <Button label="Add task" @test_id={"checklist.add"} on_click={add} />

The id goes on the node's accessibility node, which is how be-sdk-test, `drive` and the host
tests find it. The convention is `<editor>.<what it does>`, keyed for repeated items by
whatever the block keys them by (`checklist.item.{id}.done`, with the item's `ObjectId`), not
by drawing order.

An id counts only on a node in the tree: a copy hidden by a `Show`, a dock panel not shown or a
closed overlay does not. Two nodes in the tree with one id fail any test that asks for it, so a
control drawn in two places gives each copy its own id.

## 3. Write the test

Tests live in the editor's crate, one test per file under `src/tests/`
(`./scripts/verify` splits them).

    let mut test = BeuiTest::<ChecklistRoot>::editing(Uuid::new_v4());
    test.hold(None, ChecklistContent::default());
    test.run();

    test.click("checklist.draft");
    test.run();
    test.text("buy milk");
    test.run();
    test.click("checklist.add");
    test.run();

    let items = test.content::<ChecklistContent>(None);
    assert_eq!(items.root().items[0].text, "buy milk");
    test.snapshot("adding_an_item_puts_it_on_the_list");

`BeuiTest` is the host: it runs the editor through the app's own session (`be_sdk::headless`)
over a be-wasm-ops connection encoded as on the wire, so a key the protocol cannot carry never
reaches the editor and a call the app refuses fails the test. It holds each block's content
(`hold`), applies the editor's operations and sends them back as the host would, and keeps what
else the app sent (the `take_*` methods). Give it host-side state (view, chrome, a drag) through
its own calls (`set_view`, `set_chrome`, `drag_block`): setting it on the `EditorHost` directly
skips the protocol, and the session overwrites it on the next frame.

`BeuiTest` has no hosted or floated window for an editor's main view (`module.window(view.id)` is
none), so `ModuleProgram::update`'s fit-window path (`fit_window_to`, and the repaint when the fit
changed during the frame) cannot be unit tested there. Cover it with a host test that opens the
window, and with `stress`.

### Gestures

- A gesture only queues events: nothing it causes shows in `shown()`, `label()` or the model
  until the next `run()`. Call `run()` after each gesture.
- Typing goes to what has focus, so click a text field first.
- `click()` first scrolls a target that a scroll hides or cuts into view, as a person would,
  so a test need not know how much fits on screen. It panics when no node has that id: usually
  one never given an id, or a row a virtualized list has not built.
- Gestures are drive's (`Gestures` in crates/beui-layer-drive). After a `tap`, keep touching:
  a mouse arriving after a finger can change what a phone layout shows.

### Waiting and time

- Work on another thread races the frame: a snapshot after a few `run()`s passes on an idle
  machine and fails under load. `settle_until(what, predicate)` paints until the predicate
  holds, waking on the editor's `Waker` or requested delay; work that lands without waking the
  editor stalls it until the deadline, as it would in be-host.
- `settle()` paints until a frame stops asking for another at once. Call it before a snapshot
  of something that animates in.
- The editor's clock (`timer::now()`) is the harness's: each frame is one frame interval
  later, so an animation lands on the same frame everywhere. `advance(by)` moves it on and
  paints; `wait(by)` moves it on and delivers queued host calls without painting.
- The date and time of day are pinned too, to `beui_core::timer::TEST_WALL_CLOCK`: editors
  read `be_sdk_beui::wall_clock()` and beui `beui::reactive::wall_clock()`, never
  `SystemTime::now()`, which would paint differently every run.

### The region

The region is 800×600 points at scale 1; `with_size`, `with_scale_factor` and `set_raster`
change it (a phone, a high-density screen, a VR panel drawn finer than it is laid out). An
editor whose manifest claims `pan_and_zoom` calls `in_viewport()`, and the harness then holds
a zoom and offset and answers pan, zoom and fit as the host does (guides/pan_and_zoom.md).

## 4. Snapshots of the painting

`snapshot(name)` writes what the editor painted to `snapshots/<name>.paint` beside the crate's
Cargo.toml: the shapes and glyphs beui's renderer would hand the GPU, each glyph with its own
coverage image, compressed to a few kilobytes and compared exactly
(decisions/paint_snapshots_instead_of_screenshots.md). `record()` keeps the current frame, and
`snapshot` then writes every kept frame, so a test can show how the editor got somewhere.
Every frame is compared, so keep recordings to the frames that say something.

- A painting never holds glyph atlases, so text an earlier frame drew cannot change it. Text
  that varies in the captured frame (a uuid, a temp path, the time) does: keep it out of the
  frame.
- What a beui `Drawing` draws (an app's surface, a 3D scene) never reaches the painter: the
  painting keeps the region and nothing inside it, so assert on the block instead.
- A frame is tidied before it is kept (beui-paint-format's `Frame::tidy`): what paints no pixel
  (a shape under a later opaque rect or over its own colour, a clip that cuts nothing) is
  dropped or made plain, so a change only those see leaves the painting's bytes alone. Tidying
  must never change a pixel `rasterize` paints.
- The format is beui-paint-format's types as CBOR. Grow it by adding (a variant, or a field that
  skips its default) so existing paintings keep their bytes; renaming, removing or reshaping a
  field bumps `VERSION` (beui-paint-snapshot's format.rs) and verify rewrites every painting.

### Out of bounds

`snapshot` and `record` also fail on what the frame shows out of bounds (beui-core's
`Document::overflows`): a node cut off by a clip that no scroll or pannable canvas can move it
out from under, text drawn outside its box, or an outline (a focus ring) cut by a clip while
its frame shows whole. Each line names the node, its nearest ancestor with a test id and both
rectangles. Fix the layout, not the test: wrap or ellipsize the text, let the row shrink
(`<ListItem shrink=1.0>`), scroll it, put padding inside the scroll, or give a scroll
in a padded surface `bleed` so a ring at its edge has room. beui-demo's
`tabbing_through_every_page_shows_each_focus_ring_whole` checks every page's focus rings.

### Accepting and reviewing paintings

- `./scripts/verify` accepts new and changed paintings, fails on one a test names but never
  wrote, and deletes paintings no test compared (`--check` fails instead). CI on a pull request
  does the same and pushes what it wrote.
- Git never merges a painting. On a conflict take either side (`git checkout --theirs`) and let
  verify repaint it.
- Changed paintings need no explanation in your work: a person reviews them on the pull
  request through the page its summary comment links (crates/be-tool-paint-review).
- To look at a painting you did not expect to change:
  `./scripts/buck run //crates/be-tool-local:bin -- changed` writes before, after and diff PNGs
  into `target/changed-paintings`.

### Documents that are not editors

`be_sdk_test::DocumentTest` paints any beui document (crates/beui-demo paints every demo page
this way). It is beui's `testing::Stepped` (guides/beui/core.md, Test and verify changes), running the real `Runner`
with drive's gestures and a frame clock. `click` and `scroll` reveal their target first and
then move the pointer out of the window, so a painting shows hover only where the test hovers.
`settle()` never finishes on a page that animates forever, such as a spinner. `record()` keeps
the current frame for a recording, as an editor's does.

### Browser pages

A page rendered as HTML (crates/be-server-portal-ui's portal) is snapshotted with
`beui_paint_snapshot::BrowserSnapshot`:

    BrowserSnapshot::new([390, 844], &html)
        .file("/style.css", "text/css", css.as_bytes())
        .snapshot("sign_in_phone.browser");

The `.browser` name makes the painting a text file of the viewport, the document and every
resource it loads (beui's fonts excepted), one token per JSON line so Git diffs read well. Tests
compare the bytes without a browser; crates/be-tool-browser-rasterizer renders them for review
and says how to set it up locally. Fix dynamic ids, form tokens and times in fixtures.

## 5. Running them

An app's tests are compiled to wasm32-wasip1-threads and run by crates/be-sdk-test-runner,
the same wasmtime runtime be-host opens apps with, so what they paint is what the shipped app
paints (decisions/app_tests_run_as_wasm_guests.md). A native build or test of an editor package
checks something that never ships. The host's own crates (be-host and its desktop and
accounts crates, beui-layer-space) paint natively with `cargo_test(paints = True)`. Each crate's
tests run in exactly one of these places.

    ./scripts/test //crates/be-app-checklist:test               # compare
    ./scripts/test //crates/be-app-checklist:test --update      # accept
    ./scripts/test //crates/be-app-checklist:test some_test     # one test

Both run on a worker and print everything the tests wrote, passing or failing; a passing run is
cached until something it reads changes. Under `./scripts/verify` a failure's output is in the
build log.

- A module's tests run one after another on one thread, in name order, so per-thread state
  (the frame clock, the pinned wall clock) carries from one test into the next; anything a
  harness starts afresh must be reset when it starts (`HeadlessApp::new` restarts the frame
  clock). A test that fails only after others ran passes alone: reproduce it by running the
  whole module.
- A panic aborts the wasm guest. The runner then reruns each test in its own instance and lists
  every failure with its panic message and call stack (function names, no line numbers: wasm
  has no Rust backtraces).
- Each module is precompiled to a cached `.cwasm` once, so a rerun takes seconds. The runner
  opens a GPU adapter only when a test calls the gpu ABI; nothing a test compares passes
  through a GPU.

## 6. Testing editors together

An editor test fakes the host around one editor. What happens between editors (a child taking
its parent's frame, a dialog over a child pane taking the press, a tab closing its editor) lives
in be-host's compositor and app host, and is tested in crates/be-host-tests against be-host
itself: headless, with the dev workspace open as `:dev` has it, in a data directory of its own,
driven with `drive`'s commands (guides/running/drive.md), painting the whole window with every
app in it. A test fails unless the host exits cleanly at `close()`.

    let mut host = HostTest::start(Options::default());
    add_to_my_files(&mut host, "Canvas");
    add_to_canvas(&mut host, "Checklist");
    host.crop("#workspace.block");
    host.record();
    host.click("#checklist.draft");
    host.type_text("buy milk");
    host.click("#checklist.add");
    host.wait("CheckBox \"buy milk\"");
    host.snapshot("a_checklist_on_a_canvas_works_in_place_and_zooms_with_it");
    host.close();

    ./scripts/test //crates/be-host-tests:test [NAME] [--update]

- Every step settles as `drive` does, and the clock is stopped, so carets and animations paint
  the same every run; time passes only with `pause`. A pause runs a frame only once
  the apps have answered the one before (they work on threads of their own), so an animation
  that spans a pause ends under load as it does idle. `HostTest`'s doc comments cover cropping, and
  `Options` covers the desktop, previews, space and rendering.
- Paintings are `drive paint` and fail on overflow (`drive overflows`) in the host or any app
  shown, whatever the crop.
- Each test names the apps it uses in `APPS` in crates/be-host-tests/BUCK (the comment there
  says how), so it builds and reruns only for those. A block whose app is missing fails within
  seconds, naming the app. A painting that changes when only the staged apps changed means the
  test needs another editor (a row offers "Opens in" only for a type two editors edit).
- Every test runs the one `//crates/be-host:bin` (`BE_HOST`), with `BE_BUNDLE`, a path list as
  `PATH` is (`be_lib_build_id::bundle_directories`), naming where it finds its apps: the
  `:host-with-<apps>` directory, which holds only the apps the test names, then `:host-libraries`,
  which has every library once for all of them. Staging a host per test with its own binary and
  libraries took 800 MB each.
- Each file under src/tests/ is its own `:test-<file>` (`run_apart`), which `./scripts/test`
  picks by name; it runs its one test with `--exact`, so the name `./scripts/test` adds as a
  filter runs nothing else.
- These tests take a second or two each and run in parallel, each as its own action, so a
  failing test loses only its own paintings. Use them for what only the host shows; test an
  editor's own behaviour in its crate.

### Devices of one account

`HostTest::start` has each host embed a private server. A test of what happens between devices
(sync, going offline, restarting) owns one instead: `Server` (crates/be-host-tests/src/server.rs)
runs be-server in the test's process on a directory of its own, and hosts join it as devices.

    let server = Server::start();
    let dev = server.account("Dev");
    let mut laptop = HostTest::device(&dev);
    let mut phone = laptop.another_device();
    phone.offline();
    add_to_my_files(&mut laptop, "Text");
    laptop.synced();
    phone.online();
    phone.synced();
    phone.wait("TreeItem \"Text\"");

- `Server::account(name)` registers an account; a second account is a second call.
- `HostTest::device` starts be-host with `--dev-workspace=URL` and a token the server issued that
  device (`BE_DEV_TOKEN`), in a data directory of its own; an account's first device sets its
  key. `another_device` starts one more and pairs it, typing the code its unlock screen shows into
  the first, as a person adding a device does.
- Each device reaches the server through a relay of its own (be-block-fault-relay), which is its
  network: `offline` cuts its connections, refuses new ones and returns once the device says it
  is not connected (`drive offline`); `online` gives it back and sends
  `drive network`, so the device reconnects at once instead of when its wait for the next try runs
  out. `Server::offline` cuts every device's relay at once, without waiting for any of them to
  notice. `unreachable` takes the network away without a word, as a network that drops what
  it is sent: connections stay open and carry nothing, and the device notices only when its ping
  goes unanswered, so it shows what waits on a server that stopped answering. `online` ends it
  too. Object bytes go to the server's own address,
  not through the relay, but a device asks for their URLs over its connection, so an offline one
  moves none.
- `restart` closes the host, failing unless it exits cleanly, and starts it on the same data. A
  device that was offline starts offline.
- `synced` is `drive synced` (guides/running/drive.md, Waiting): connected, nothing waiting to be
  sent, and everything it missed asked for. What another device changes arrives by push after
  that, so wait for it to show. Never pause for sync.
- Each device is a host: keep a test to two unless it needs more.

Debugging:

- A failure prints the steps so far as `drive` commands, which replay against
  `./scripts/buck run //crates/be-host:dev -- --fresh`, plus `drive tree` and the end of the
  host's log with every panic in full. A step that times out while the tree shows nothing
  pending means something asks for a frame every frame.
- A flake that shows only under load: `./scripts/buck run //crates/be-tool-local:bin -- stress NAME
  [--repeat N] [--parallel K] [--limit SECONDS]` builds the host tests' harness once and runs
  the tests whose names contain NAME N times (20), K at a time (twice the cores), killing a
  run that takes more than SECONDS (300). It prints how many runs failed, grouped by the
  panic's message with the numbers blurred and the first logs of each group, which are under
  target/stress/NAME/. Every run uses the host that has every app, not the apps a test names
  in `APPS`. `./scripts/test` caches a passing run, so repeating it proves nothing. More at
  once slows every step down, but at 7 or more per core single steps pass 30 seconds with
  nothing wrong. Report a fix with its failure counts before and after, such as "3 of 200
  before, 0 of 800 after": a rate of one in a few hundred needs hundreds of runs.
  `CHANGED_PAINTINGS=DIR UPDATE_SNAPSHOTS=1` writes each painting that differs from the
  accepted one to DIR with what changed in a `.why` beside it, so a painting that flips keeps
  the variant to compare (`rasterize`, then compare the pictures).

Writing a test that does not flake (decisions/host_tests_stay_flake_free.md says why these):

- Wait for what the test needs, never for time. Between devices that is `synced` and then a line
  of the tree (above). A `pause` runs frames, and runs each only
  once the apps have answered the one before it, so an animation that spans one ends the same
  under load; it is not a way to wait for a thread, a server or the network. For those use
  `wait` and `wait not` on a line of the tree, or on a node's place (`wait not ID over ID+X,Y`).
- Whatever the pointer rests on is part of the painting. A press that opens a menu or a card
  under the pointer leaves it over the new content, and whether that is highlighted depends on
  how the frames fell. A painting that is not about hover moves the pointer away first
  (`host.run(&["leave"])`); one that is about hover puts it there on purpose (`move`).
- Take a painting only of what has settled: `paint` waits until the host, the apps and the
  store have nothing left to do, and fails with what kept them busy. `HostTest::record` (so
  `snapshot`) paints twice with a settle between and fails with "the painting is not settled"
  if the two differ, whatever the test did: a hover, an animation's last step or an app's late
  frame is caught the first time it shows, on any machine. Wait for it, not for time.
- `drive pause 200` before a `shot`, as drive.md says, is for a hosted Wayland or X11 program,
  which draws on its own after input; it is not a way to wait in a host test.
- Do not read the clock, the machine's speed or the order threads finish in. A test that
  passes alone and fails 1 run in 200 under `stress` is a flake whatever its author thinks.
- Run `stress` on a new host test, 100 runs at 14 at a time, before sending it. On a
  machine with fewer than 7 cores, 14 at a time is more than 2 hosts per core, and a few
  runs in a hundred then time out at the very first `wait` (the Files pane still showing
  "Loading the files" after 30 seconds, nothing wrong): they pass at `--parallel` equal to
  the cores. Count such a timeout as load, not as the test's flake, and raise nothing.
- A hang: `gdb -p PID -batch -ex 'thread apply all bt'` on its be-host. Every thread parked with
  the window waiting on data means something changed without `host::wake()`: a lost wake, not a
  deadlock.

## 7. Testing both of beui's cores

beui has a native core and an HTML core whose primitives are DOM elements
(guides/beui/core.md, Cores; decisions/beui_has_an_html_core_on_the_web.md). crates/beui-tests
holds what must behave the same on both: screens (each a `#[component]` in src/lib.rs that
`Fixture` picks by name) and one `drive -` script per screen, `src/drive/NAME.drive`.

A script uses only the commands both cores answer (listed in crates/beui-tests/src/lib.rs).
Text metrics differ between the cores, so assert with `wait` and `wait not` on test ids or on text
the screen writes itself, never on coordinates: to check a layout, have the screen print what
it `measure`s in a `Text` (`wait 'Inner 50x20 at 140,50'`). To cross a timeout, `clock stop` and
`pause`.

- Natively, `./scripts/test //crates/beui-tests:test` runs each script against `:bin` with no
  GPU, once as is and once with every measurement a frame late (`--delay-measurement`), so a
  screen that relies on synchronous measurement fails.
- In a browser, `./scripts/test //crates/beui-tests:web-test [NAME]` serves the screens built
  for `wasi_html` and runs every script in Chromium through Playwright (buck/program/web-test.mjs,
  `web_drive_test` in buck/program/drive.bzl); verify runs it as `:web-test[run]`. Reading
  commands go to beui-core-html's `automate`; input is real Playwright input aimed where
  `find TARGET reveal` says, so the browser's own focus, key and IME paths run. It also runs the page's
  `checks()` (src/web/live.rs). It runs on a worker, like any test, with Node, Playwright and
  Playwright's headless Chromium pinned in buck/tools (the versions buck/program/browser-tests'
  lockfile names) and Chromium's libraries from `buck/sysroot:amd64-browser`; it fails rather than
  skipping when the browser is missing. To run it by hand, set `BE3_CHROME` to a Chrome or
  Chromium and put Node's `playwright` package on `NODE_PATH`.
- A script both cores run marks a step only the browser can make with `#web COMMAND` (a comment
  natively): the browser-only commands are `attribute`, `history`, `history-state` (the current
  history entry holds the page's own back entry or not), `loads` (how many times the page has
  loaded), `reload`, `selection`, `highlighted` (what the page's CSS highlights cover),
  `elements` (how many elements match a selector), `role` and `roles`; web-test.mjs's header
  says what each takes.
- To see a screen: `./scripts/buck run //crates/beui-tests:dev -- NAME` and `drive`; in a browser,
  serve `:web` with COOP `same-origin` and COEP `require-corp` and open
  `index.html?fixture=NAME`.
- To cover a component on both cores: add a screen to src/lib.rs, its name to `Fixture`, the
  script `src/drive/NAME.drive`, and `src/tests/NAME.rs` calling
  `drive("NAME", include_str!("../drive/NAME.drive"))`. The web test finds the script itself.
