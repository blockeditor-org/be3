GUI tests run headless: no window, no input, no server. A test builds an editor, drives it the
way a person would, and checks two things — what the block became, and what the editor
painted. They are fast enough to belong in ./scripts/verify: the handful that exist run in
well under a second.

A plugin's tests run where the plugin runs: compiled to wasm32-wasip1-threads and started by
the same wasmtime host the app opens a plugin with (see 5). Nothing about writing one changes
because of that, but what the editor paints is then what the shipped plugin paints rather than
what the machine running the tests happens to have installed.

1. What a GUI test may look at

- The block. An editor's job is to turn gestures into operations, so the assertion is on
  the content the operations reached once the test, standing in for the host, applied them.
- The painting, as a snapshot (see 4) - one frame, or a recording of several. This catches
  what an assertion on the block cannot: a control that vanished, a panel that lost its
  contents, a colour that changed.
- Never a coordinate, a widget's size, or the order of the accessibility tree. Those change
  whenever anyone touches a layout and say nothing about whether the editor works.

2. Give the widgets test ids

Widgets are found by an id the test names, never by their label: renaming a button, giving
it an icon, or moving it to a sidebar then leaves the tests alone.

    <Button label="Add task" @test_id={"checklist.add"} on_click={add} />

@test_id names the node the component builds. The node's accessibility node carries the id,
so the frame reports the rectangle of every named node the accessibility tree shows, which is
what block-ui-test clicks. Name them
`<editor>.<what it does>`, and where there are many of a kind, key them by whatever the
block itself keys them by (`checklist.item.{id}.done`, where `id` is the item's `ObjectId`,
the same id the edit names), never by the order they happen to be drawn in. An id names one node in the tree:
a copy hidden by a Show, a panel a dock is not showing, or a closed overlay does not count, and
two nodes in the tree with the same id fail the test that asks for it, so a control drawn in two
places (a toolbar and a dock, say) gives each copy its own id.

3. Write the test

Tests live inside the editor's crate, one test per file under src/tests/, like every other
test in the repository. block-ui-test's BeuiTest is built from an OwnedEditor, which owns the
state behind the Copy Editor the view is handed (it derefs to one, so `let editor = *owner;`
keeps a handle after the BeuiTest takes the owner), and runs it headless through the plugin's own session (block_editor_plugin::headless):
the Screens and EditorSession the shipped plugin runs, fed the calls the app's host would
make: the world's exports and the answers to the plugin's imports over a be-ops connection, as
on the wire, and what the host pushes (acceptance, preferences, input, the draw) encoded and
decoded as the wire would. Gestures are turned into the protocol's input events the way
block-app turns beui's, so a key or a modifier the protocol cannot carry never reaches the
editor here either, and a call the plugin refuses fails the test.

    let block = Uuid::new_v4();
    let host = EditorHost::default();
    host.set_editable(true);
    let editor = OwnedEditor::new(host, block);
    let mut test = BeuiTest::<ChecklistApp>::new(editor);
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

There is no server and no block store in an editor test: BeuiTest is the host, and does
what the app's host does for the editor's block. hold(block, content) gives it the content of
a block - None for the editor's own, Some(id) for one the editor watches with content_of,
which it only hands over once the editor has asked for it - and run() paints a frame, applies
every operation the editor sent to what it holds, and sends them back as operations marked as
the editor's own, until nothing changes: the first content a block gets is a snapshot and
everything after it an operation, as in the app. content::<C>(block) reads what a block holds
now, edit::<C>(block, &operation) is an edit arriving from someone else, and seeded() is the
content the editor seeded or replaced. store() is the ContentStore behind it, which also
stands in for the graph: own and add_block put blocks in it, block reads one back, created
lists the blocks the editor made through its Blocks handle, and defer(true) holds the
editor's operations back until defer(false), for a test of what the editor shows before the
host has taken an edit.

Whatever else the plugin sends the host is kept for the test to read: take_view_changes(),
take_requests(), take_opens(), take_block_commands() (blocks.act), take_shell_commands() (shell.act: share, leave, rename, close, the app menu) and the other take_ methods drain one
kind, take_host_calls(reader) the calls of the account and extension interfaces it made (block_ui_test::requested has a reader per function), and clear_calls() drops what is kept. The host's side of a conversation goes in the same way the
app sends it: set_view, set_chrome, with_top_bar, drag_block, answer, set_histories,
set_peers, report_children, resize and presence_visible each queue the call the app
would make. Setting the same state on the EditorHost directly skips the protocol, and the
session overwrites what it owns - the chrome, the view, a drag - on the next frame.

A gesture only queues events: click() returns before the editor has seen the click, so
nothing it does (a handler's effect, a row added, a dialog opened) shows in shown(), label()
or the model until the next run(). Call run() after every gesture — the view only sees an
event on the frame it is delivered in — and click a text field before typing into it. click() first scrolls a target that is out of view into it
(`reveal`, every enclosing scroll, as a person would) and runs a frame, in BeuiTest and
DocumentTest alike, also when a scroll only partly shows it, such as a row a menu's
scroll cuts (`Document::is_cut`), so a test need not know how much fits on its screen; drive does the same
for its targets (guides/running/drive.md, Targets). It panics when no
node has that test id, which is usually a node that was never given one, or one a virtualized
list has not built; shown() and label() ask about one without clicking it.

run() hands the editor what was queued since the last one, frame by frame. Its gestures are
drive's, built by the same `beui_host::native::drive::gestures::Gestures` (guides/running/drive.md),
so an editor test presses, types and touches as drive and the app tests do: the pointer spends
a frame arriving where a click, a press or a drag starts before it presses, a click or a key
then arrives in one frame, a key carries the text it types and a chord presses its modifiers
as keys of their own, text() types key by key, and drag(from, to) is the press, a move halfway
and the last move with the release. drag_along(points) spends a frame on each point and lets
go in a frame of its own, for a gesture whose path matters, like a lasso, or one that should
come to rest where it ends, like a sheet dragged by its handle. tap(test_id) is a finger's
click: a test that touches keeps touching, since a mouse arriving after a finger can change
what a phone layout shows. next_frame() ends the frame being queued, and step(events) paints
one frame with exactly the events it is handed.

An editor that hands work to another thread paints a spinner until the work lands, so which
of the two a test captures is down to whether the thread beat it to the frame: a snapshot
that passes on an idle machine and fails when the suite is running thirty editors at once.
settle_until paints until a predicate over the harness holds, and fails the test rather
than the painting if it never does. Between frames it waits the way the app does: for the
editor's Waker, or for the delay the editor asked to be painted again after, and never for a
fixed time. Work that lands without waking the editor and without asking for a frame stalls
it until the deadline, which is the stall a person would see in the app.

Time in the editor is the harness's own clock, not the wall clock: every frame the harness
paints is one frame interval later, and the host hands the plugin that time with each frame,
which is what its frame clock (`timer::now()`) reads. An animation therefore lands on the same
frame on any machine, a frame captured part way through one paints the same everywhere, and
waiting for a delay the editor asked for costs no real time. advance(by) moves the clock on and
paints a frame, for a timer that is longer than an animation:

    editor.advance(Duration::from_secs(60));

    editor.settle_until("the lighting to land", |editor| editor.shown("scene.lit"));

wait(by) moves the clock on without painting and hands the plugin what the host has queued
(a set_watched, say), the way a call reaches an idle editor between frames; the next
run() paints the frame that takes it. Whatever the editor's effects do with it at the top of
that frame reads that frame's time from `timer::now()`, not the last frame painted.

settle() is the same wait for the editor's own animations, such as a sheet sliding in:
it paints until a frame stops asking to be painted again at once. Call it before a
snapshot or record() of something that animates in.

Wait for the work rather than for a number of frames: a few more run() calls is the same
race with a wider margin, which is how one of these hid.

The date and the time of day come from the harness too: an editor reads them from
`block_editor_beui::wall_clock()` (and `utc_offset()` for local time), never
`SystemTime::now()`. That clock is the host's: each draw carries the host's wall clock beside
its frame time (`wall-micros`), and the harness hands one fixed moment plus its frame clock, so
a clock or a calendar paints the same on every run. beui's own date and time components (a
calendar's today, a picker's now) read `beui::reactive::wall_clock()`, which the plugin runner
pins to that clock each frame; a beui test without a plugin pins it with
`beui::reactive::pin_wall_clock(Some((timer::now(), since_epoch)))`. The fixed moment is
`beui_core::timer::TEST_WALL_CLOCK` (2026-03-02 09:41 UTC). The app tests and every other test
that starts a program through `be_drive::Driven` or `be-drive run` start it with its clock
pinned (`be-drive launch --pin-clock`, guides/running/drive.md): the headless platform pins
beui's wall clock to that moment as it starts and runs the app in UTC, so block-app hands every
plugin that moment too. An agent's `:dev` runs on the real time unless started with
`-- --pin-clock`.

An editor whose manifest claims pan_and_zoom draws into a view the host owns, so its test
calls in_viewport() on the harness. The harness then does what the host does around the main
region: it holds a zoom and an offset, hands the editor a view over the content band the
editor reported (the whole region until it reports one), and answers
the pan, zoom and fit the editor asks for, fitting the content until the first of them
arrives. An editor that is not in a viewport is told nothing about a view and fills its
region, which is what an editor without that capability does anyway.

The region is 800 by 600 points at a scale factor of 1; with_size(Vec2::new(390.0, 800.0))
places it the size of a phone, with_scale_factor(2.0) draws it the way a high-density
screen does, and set_raster(3.0) draws it finer than it lays it out, as a panel in VR is.

4. Snapshots of the painting

editor.snapshot(name) writes everything the editor painted into
snapshots/<name>.paint in the crate's own directory, beside its Cargo.toml, which holds every
painting the crate's tests accepted: the rounded rectangles, glyphs and triangles beui's
renderer would hand the gpu, each glyph carrying its own coverage image, compressed. It is a few kilobytes rather than the hundreds a screenshot
costs, and it is compared exactly, so nothing about it is flaky.

A painting is a recording: one frame by default, or the frames the test kept. record()
keeps the frame the editor has just painted, and snapshot(name) writes the frames kept
since the last one — or the frame the test is on, if it kept none — so recording a gesture
at a time paints how the editor got somewhere rather than only where it ended up.

    editor.record();
    editor.click("checklist.add");
    editor.run();
    editor.record();
    editor.snapshot("adding_an_item_puts_it_on_the_list");

The frames of a recording share one table of textures, so a frame that draws the same text
as the last costs the triangles that draw it and nothing more. Keep recordings to the
frames that say something: every frame is compared, so a frame nobody looks at is one more
way for the test to fail.

- ./scripts/verify accepts whatever the tests paint: it runs them with UPDATE_SNAPSHOTS=1, so
  a new or changed painting is written into the crate's snapshots/ rather than failing the run. On a pull
  request CI does the same, and when that writes a painting it fails the run and pushes the
  painting to the pull request's branch as a commit. Everywhere else CI runs ./scripts/ci --check, which sets nothing,
  so a painting that was never committed fails there.
- Git never merges a painting: when two branches change one, it conflicts. Take either side
  (`git checkout --theirs` or `--ours` on it) and let ./scripts/verify repaint it.
- ./scripts/verify also fails on a painting that a test names with a literal, snapshot("name"),
  but that the crate's snapshots/ does not hold once it is done: the test failed, or returned, before
  painting it.
- Once every plugin test passes, ./scripts/verify deletes each painting in a snapshots/ that no test
  compared, so renaming or removing a snapshot takes its old file with it; with --check it
  fails on them instead. A single editor's test run leaves the folder alone.
- A changed painting is for a person to review, not for you. They review it on the pull
  request: its summary comment links to a page (crates/paint-review, served on the web by
  crates/paint-review-web) that lists every painting the pull request added, removed or
  changed, and flips each frame between before and after. The comment also shows up to 30
  frames inline, with before, after and highlighted changes, inside one collapsed disclosure.
  The review page uses tiled
  docking on desktop and stacked docking below 700 points: select a painting to open
  its view, then use the dock back button to return. Images fit the stage initially;
  pinch, scroll or middle-drag to inspect them, and use Fit or 100% to reset the view.
  At 100%, each image pixel occupies one UI point (one CSS pixel on the web), regardless
  of the screen’s device pixel ratio.
- snapshot(name), and every record() before it, also fails on what the frame shows out of
  bounds (beui-core's `Document::overflows`): a node cut off by a clip that no scroll,
  pannable canvas or canvas its owner scrolls across (`Canvas scrolls_across`, as a single-line
  text field's) can move it out from under - the region's edge, a sidebar too narrow for a
  row of controls - text drawn outside its own box, and an outline drawn outside a frame (a
  focus ring) that a clip cuts while the frame itself shows whole. A node culled out of sight is
  checked only for being cut off, since its text keeps its last shape until it is laid out in
  sight again. Each line names the node, its
  nearest ancestor with a test id, and the two rectangles. Fix the layout - wrap or ellipsize the
  text, let the row shrink (`ItemSize::Intrinsic.shrink(1.0)`), scroll it, put the padding
  inside a scroll around its content, or give a scroll inside a padded surface `bleed`, so a ring at its edge has room - rather than the test.
  beui-demo's `tabbing_through_every_page_shows_each_focus_ring_whole` tabs through every
  catalog page and fails on any such ring, since a painting rarely has the focus showing.
- ./scripts/buck run //crates/local-tools:bin -- rasterize <crate directory>/snapshots/<name>.paint out.png
  turns one into a PNG, and a trailing frame number or all picks which frames of a recording
  to write.
- A painting is paint-format's types as CBOR (`paint_format::encode`), deflated behind a
  version number (`VERSION` in paint-snapshot's format.rs). Accepting compares the CBOR before
  deflating it, so a painting passes or fails on what it holds. Grow the format the way
  guides/frozen_formats.md grows CBOR values - a new enum variant, or a new field with
  `#[serde(default, skip_serializing_if = ...)]` that skips its default - so the paintings that
  do not use it keep their bytes. Only renaming, removing or reshaping a field bumps the
  version, and then ./scripts/verify rewrites every painting.
- Regenerating them is cheap and mechanical - a change to beui's renderer rewrites every one - so
  you do not need to review or explain changed paintings. They show up on the pull request,
  and a person reviews them there before merging. Leave the images alone.
- The exception is a painting you did not expect to change: if your change should not have
  touched it, find out why it changed.
  ./scripts/buck run //crates/paint-snapshot:example-changed says, for every native painting in
  a crate's snapshots/ that differs from HEAD, which frame changed and what moved in it. With
  -- --images it also writes them into target/changed-paintings as PNGs - a before, an after
  and a changes image for every frame that differs, where magenta marks every pixel that
  differs - and prints which is which. --base compares against another revision, and
  --images takes a folder to write to in place of target/changed-paintings.

A beui document that is not an editor - beui's demo, say - is painted the same way through
block_ui_test::DocumentTest, with only the fonts beui carries, never the system's. Its tests
are compiled to wasm with plugin_tests like an editor's; crates/beui-demo paints every page of
the demo that way. A DocumentTest is beui's `testing::Stepped` (guides/beui.md, Test and
verify changes) with snapshots added: the document runs through the real `Runner`, frame(inputs)
pushes raw `Input`s in points through the runner's intake as a platform would, and its
gestures are drive's - `click`, `drag`, `scroll`, `key`, `type_text`, `fingers` - each
answering after the frame that took it. `click` and `scroll` reveal a test id that is out of
view first and move the pointer out of the window afterwards, so a painting shows hover only
where the test hovers. Its clock is a frame clock too: every frame is one frame interval
later, and advance(by) moves it on without painting. settle() waits as drive does, which a
page that animates forever, such as one showing a spinner, never finishes.

A snapshot never holds the glyph atlases. Each glyph carries its own image - coverage, or
colour for an emoji from a colour font - keyed by what is in it, so where a glyph happened
to land in an atlas cannot reach the file: text an earlier frame drew - a temporary directory's name, a uuid, the time - repacks the atlases
without moving anything in the snapshot. Text that varies in the frame the test captures is
of course a different painting, and still has to be kept out of it.

Nor does a snapshot hold what a beui Drawing draws - a plugin's surface, a 3D scene - since
those contents never reach the painter: the snapshot keeps the region and nothing inside it,
so a test of one asserts on the block instead.

beui shapes and rasterizes its glyphs through the HarfBuzz and FreeType it carries, with
the fonts it carries, once it is compiled to wasm, which is how its tests run, so what an
editor paints is comparable like anything else.

Browser pages use `paint_snapshot::BrowserSnapshot` instead of native drawing commands:

    BrowserSnapshot::new([390, 844], &html)
        .file("/style.css", "text/css", css.as_bytes())
        .file("/script.js", "text/javascript", js.as_bytes())
        .snapshot("sign_in_phone.browser");

The name ends in `.browser`, making `snapshots/<name>.browser.paint` a text file.
It contains the viewport, the complete SSR document and every embedded resource, with
DOM tokens and resource lines on separate JSON lines for readable Git diffs. Tokens
are concatenated without adding whitespace, preserving the exact rendered document.
Binary resources such as images can be supplied through the same `file` method.
Resources use absolute paths or full URLs; the document is served at
`https://snapshot.invalid/`. Include resources referenced by CSS or scripts as well,
except beui's bundled fonts: the rasterizer serves each file in `beui_fonts::FACES`
(`/Inter-Regular.ttf`, `/lucide.ttf`, ...) at the origin's root unless the painting embeds
that path, so a page on beui's HTML core whose `<style>` is `beui::html::stylesheet()`
draws its text and icons without copying the fonts into every painting.
Keep dynamic ids, form tokens and displayed times fixed in fixtures.
Native test crates opt into acceptance and cleanup with `cargo_test(paints = True)` in
BUCK. The portal's tests in `crates/be-portal-ui/src/tests/` demonstrate this.

Tests compare the encoded bytes without starting a browser. Browser paintings share the
native acceptance and cleanup pipeline, but Git shows their text diffs and prevents line
merging. GitHub's painting comment and `local-tools rasterize` render them on demand through
`crates/browser-rasterizer`, a separate binary.

For local browser rasterization, install the optional runtime once:

    python3 -m venv ~/.cache/be3/browser-rasterizer
    ~/.cache/be3/browser-rasterizer/bin/pip install -r crates/browser-rasterizer/requirements.txt
    ~/.cache/be3/browser-rasterizer/bin/python -m playwright install --with-deps chromium
    export BE3_PLAYWRIGHT_PYTHON=~/.cache/be3/browser-rasterizer/bin/python
    ./scripts/buck run //crates/local-tools:bin -- rasterize crates/be-portal-ui/snapshots/portal_login_phone.browser.paint out.png

Setting `BE3_CHROME` to a Chrome or Chromium executable uses that browser instead of
Playwright's download, which is what CI does with the runner's Google Chrome.

`local-tools` builds the browser rasterizer only for browser inputs. During CI previews,
that binary is built by the same buck2 invocation as the other verification tools.
The runtime blocks all uncaptured network requests, waits for fonts and images, disables
animations and reports missing resources or failed scripts instead of silently producing
an incomplete preview. Browser rendering is not part of snapshot equality.

5. Running them

./scripts/verify runs them, through buck2: ./scripts/verify --plugin-tests is the plugin
tests alone, and accepts what they paint. buck2 compiles each plugin's tests for
wasm32-wasip1-threads against the WASI sysroot the web build uses, on the build server,
and runs the module through crates/plugin-test-runner, which is wasmtime with the
plugin's own imports linked: the gpu abi a plugin draws through, the threads a plugin
spawns, and the repository itself. Under ./scripts/verify the run is an action on a worker,
which reads the accepted paintings as its inputs and outputs the ones that changed;
./scripts/verify then copies those into the crate's snapshots/, so a failure's output is in the
build's log, not on this machine. ./scripts/test (below) runs it on a worker too, and
copies what it accepts into the crate's snapshots/.
Do not build or test an editor package for the host by itself:
its tests only mean anything as the wasm guest it ships as, and a native run refuses to
compare a painting rather than making one nothing would agree with.

While working on one editor, run its tests alone:

  ./scripts/test //crates/editors/checklist:test
  ./scripts/test //crates/editors/checklist:test --update

The first compares, the second accepts. Both print everything the tests wrote, println! and
eprintln! alike, the guest's stdout and stderr each under its own heading after the target's
"✓ Pass" or "✗ Fail" line. ./scripts/buck test //crates/editors/checklist:test runs the same,
but shows a passing test's output only when BE3_VERBOSE=1 is set. A panic aborts a wasm guest, so a failing test ends
its binary's run; the runner then lists the tests and runs each in an instance of its own, and
reports every one that failed, each with its panic message and the functions it panicked in,
innermost first, at the end of the output. Rust's own backtraces are not available in wasm, so
the runner reads the call stack from wasmtime instead; it has function names but no line
numbers.
Until then the module's tests run one after another, in name order, on one thread, so what a
crate keeps per thread (the frame clock, the pinned wall clock) carries from one test into the
next, and anything that starts afresh with each harness must be reset when it does
(HeadlessPlugin::new restarts the frame clock). A test that fails only after others ran passes
alone, and in the runner's rerun too; reproduce it by running the whole module, with no filter:
./scripts/test //crates/editors/checklist:test. A single test is a filter handed through to the
test binary: ./scripts/test some_test_name finds the crate and runs it, and
./scripts/test //crates/editors/checklist:test some_test_name names the crate itself. A run
that passed comes from the cache when nothing it reads changed; one that failed runs again.

Cranelift compiles each test module once, as an action of its own, and leaves the machine
code as a .cwasm the runner maps in, so a run that changed nothing takes seconds; that
action is cached like any other, so a machine that never built the plugin gets it from
the build server. The runner only opens a graphics adapter when a test calls the gpu abi, so a
machine without one still runs every test that does not, and what a test compares never
passes through a gpu.

6. Testing editors together

An editor test hosts one editor and fakes the host around it: the children it places are only
recorded, and a child's statuses are whatever the test reports. What happens between editors
(a child taking its parent's frame and handing it back, a dialog over a child pane taking the
press, a tab closing its editor) lives in block-app's compositor and plugin host, so it is
tested in crates/app-tests instead, against the app itself. Each test there starts the native
app headless with every plugin and the dev organization open, exactly as `:dev` does, in a data
directory of its own; drives it with the commands `drive` has (guides/running/drive.md); paints
the whole window, every plugin included; and fails unless the app exits cleanly when it closes,
which close() checks, so a test that ends without calling it fails too.

    let mut app = AppTest::start();
    add_to_my_files(&mut app, "Canvas");
    add_to_canvas(&mut app, "Checklist");
    app.crop("#workspace.block");
    app.record();
    app.click("#checklist.draft");
    app.type_text("buy milk");
    app.click("#checklist.add");
    app.wait("CheckBox \"buy milk\"");
    app.snapshot("a_checklist_on_a_canvas_works_in_place_and_zooms_with_it");
    app.close();

- Every step settles as `drive` does, and the clock is stopped as the app starts, so time
  passes only with pause(milliseconds) and a caret or an animation paints the same every run.
  Keep it stopped: a closing menu or sheet goes only after a pause as long as its animation.
  A pause costs only the frames it runs, not the real time it names (guides/running/drive.md,
  `clock`).
  The wall clock is the headless platform's fixed moment (in UTC) plus the frame clock, so a
  clock in the desktop's bar paints the same too, as long as the app is ready within a minute.
  click() first waits for its target, and record() and snapshot() wait for every editor's
  loading state (`#editor.loading`) to go.
- Target nodes by test id or by a line of the tree, never by fixed coordinates: where a gesture needs a point, take it from `place(target)`, and send it with `run`, since `click` waits for its target first. Block ids are random,
  so a test id or a painting that holds one, or the order of blocks the file tree sorts by id,
  differs every run: crop(target) paints only that target's area, such as `#workspace.block`,
  the block a tab shows.
- record() and snapshot() are a painting as in an editor test (4), named
  crates/app-tests/snapshots/<name>.paint and accepted the same way. The painting is `drive paint`: the
  host's own document with each plugin's painting put where the host composites its surface,
  and each preview a plugin shows, which its painting marks with `paint_format::preview_key`,
  replaced by the painting of the preview it shows. A preview waits out its cooldown before it
  paints a second change (guides/adding_a_plugin_editor.md, Drawing the block itself), and the
  clock is stopped, so a test that changes a block and then looks at a preview of it pauses for
  `PREVIEW_COOLDOWN` first.
  They also fail, as in an editor test, on what the window shows out of bounds (`drive
  overflows`), in the host or in any plugin shown, whatever the crop.
- `AppTest::start_in_space()` starts the app in the VR simulator (`BEUI_LAYERS=space` on the
  headless platform), where the painting is the 3D view and targets and points are the main
  panel's, so it shows what happens between panels, such as a plugin's window becoming one.
- The app runs with no GPU renderer (`BEUI_RENDERER=none`), and plugins then draw nothing on
  the GPU either, since a painting needs neither. The 3D view of space and the previews of
  blocks (a slide in a deck's filmstrip) are textures, so `start_in_space()` and
  `AppTest::start_drawing_previews()` start it with the renderer, drawing through lavapipe.
- ./scripts/verify runs each test as an action of its own, so a test that fails leaves the
  paintings of the others to be accepted.
- Any failure, a plain `assert!` or a painting as much as a step, prints the steps so far as `drive` commands, which replay against
  `./scripts/buck run //crates/block-app:dev -- --fresh`, what the app shows (`drive tree`), and
  the end of the app's log, after every panic the app logged before it, each whole with its message and backtrace. A step
  that times out while the tree shows nothing pending means the app never settled: something
  asks for a frame every frame.
- buck caches a test that passed, so a flake that shows only under load is reproduced by running
  the harness on this machine, all the tests at once and as often as needed. Build it with what
  its `run_env` in crates/app-tests/BUCK names (`./scripts/buck build
  //crates/app-tests:test-harness //crates/block-app:native //buck/sysroot:amd64-test
  //buck/sysroot:amd64 //buck/sysroot:lavapipe_icd.json`), then run
  `buck-out/v2/art/root/*/crates/app-tests/__test-harness__/app_tests` with `APP_LIBRARIES`,
  `BLOCK_APP`, `VK_ICD_FILENAMES` and `XKB_CONFIG_ROOT` set to those outputs under buck-out and
  `CARGO_MANIFEST_DIR` to crates/app-tests. While a test hangs, `gdb -p PID -batch -ex 'thread
  apply all bt'` on its block-app shows what every thread waits on. If every thread is parked and the
  window waits on data, something changed without `host::wake()`: a lost wake, not a deadlock.
- Each test takes a second or two, most of it the app starting, and they run in parallel. Keep to what only the app can show, and test an
  editor's own behaviour in its own crate.

  ./scripts/test //crates/app-tests:test
  ./scripts/test //crates/app-tests:test --update

7. Testing both of beui's cores

beui has two cores (plans/html_core.md): the native one, which lays out and paints itself, and
the HTML one, whose primitives are DOM elements the browser lays out. crates/beui-shared-tests
holds what must behave the same on both: screens, each a `#[component]` in src/lib.rs that
`Fixture` picks by name, built from primitives the HTML core has, and a script per screen,
src/drive/NAME.drive, `drive -` input that drives the screen of its file name. A script uses
only the commands both cores answer: `settle`, `pause`, `clock stop|run`, `tree`, `ids`,
`find`, `wait`, `gone`, `click` (also `double`, `right`, `middle`), `move`, `down`/`up` (with a
button, to check what a gesture does while held), `drag` (with a button too), `wheel`, `type`
and `key`. It asserts with `wait` and `gone`, and names
what it waits for by test id or by text the screen writes itself, never by coordinates, since
text metrics differ: to check a layout, `measure` the nodes and print the sizes and offsets in a
`Text` (`wait 'Inner 50x20 at 140,50'`). A point a fixed distance into a node is `#TEST_ID+X,Y`,
from its top left, as in `drag '#fields.seats+10,10' '#fields.seats+16,10'`. Stop the clock
(`clock stop`) and move it with `pause` to cross a timeout, such as a toast's expiry.

- Natively, `./scripts/test //crates/beui-shared-tests:test` runs each script against the
  crate's `:bin` under `be_drive::Driven` with no GPU, once as it is and once with every
  measurement a frame late (`--delay-measurement`, as `BEUI_DELAY_MEASUREMENT=1`), so a screen
  that only works when measurement is synchronous fails. A test per script is in src/tests/.
- In a browser, `./scripts/test //crates/beui-shared-tests:web-test [NAME]` serves `:web`, the
  screens built for `wasi_html`, and runs every script in Chromium through Playwright
  (buck/app/web-test.mjs, `web_drive_test` in buck/app/drive.bzl). The page answers the reading
  commands through beui-core-html's `automate` (its tree is in native's line format, from the
  core's nodes and `getBoundingClientRect`); `click`, `move`, `down`, `up`, `drag`, `wheel` (a tick is a line, 40 pixels), `type` and `key` are real Playwright input
  at the place `reveal` answers (beui-core-html's: the part of the target that shows, once a
  scroll that cuts it has revealed it, and after a reveal every target of the command is found
  again, as native drive reruns the command), so the browser's own focus, key and IME paths run, and `resize WxH`
  sets the page's viewport, as native `resize` sets the headless window. Three commands are browser
  only, for scripts that run nowhere else (crates/beui-demo-html's): `selection none|some`,
  `role ROLE NAME [STATE...]`, which waits for Playwright's `getByRole` to find exactly one
  element with that ARIA role, exact name and states (`checked`, `selected`, `not-checked`,
  `level=2`, ...; `modal`/`not-modal` and `inert`/`not-inert` (left out of the tree by an
  inert ancestor) read Chrome's own accessibility tree over CDP, since Playwright has no such
  query), and `roles ROLE COUNT`, which waits for exactly COUNT visible elements with
  that role, whatever their names (`roles dialog 1` while one dialog is open). It also runs
  `checks()`, the live DOM backend's checks in src/web/live.rs (mounting, hydration, listeners),
  failing on any line that is not `ok`. `settle` there waits until no frame is wanted at once:
  one a timer will ask for later, such as an indeterminate progress bar's next tick, does not
  count, as natively. It prints `ok NAME` for a script that passes, and for one that fails its
  commands so far with their answers, the failure, the tree and the page's console. It runs on this machine, not a worker, which have no
  browser: `BE3_CHROME` when set, else Playwright's Chromium, through Node's `playwright`
  package (installed globally, or from buck/app/browser-tests' lockfile with `npm ci` and
  `NODE_PATH` pointing at its node_modules, as CI does); without them it fails rather than skipping.
  ./scripts/verify runs it too, as `:web-test-run`.
- To see a screen: `./scripts/buck run //crates/beui-shared-tests:dev -- NAME` and `drive`
  natively; in a browser, serve the `:web` bundle with any server that sends
  Cross-Origin-Opener-Policy `same-origin` and Cross-Origin-Embedder-Policy `require-corp` and
  open `index.html?fixture=NAME`.
- To cover a component on both cores: add a `#[component]` screen to src/lib.rs, a branch for its
  name in `Fixture`, the script src/drive/NAME.drive, and
  src/tests/NAME.rs calling `drive("NAME", include_str!("../drive/NAME.drive"))`. The web test
  picks up the script by itself.
