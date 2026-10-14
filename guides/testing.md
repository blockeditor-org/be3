# Testing

How tests are run (natively, as wasm plugin tests, and as compile_fail cases), how to write a GUI
test for an editor or a beui document, and how paintings (snapshots) are made, accepted and
inspected. Read "Running tests" to run something, "Writing a GUI test" before writing one.

## Running tests

| Command | What it does |
|---|---|
| `./scripts/verify` | every test, through each crate's cached `:test-run`, accepting new and changed paintings |
| `./scripts/verify --tests` / `--plugin-tests` | only the native tests / only the plugin tests (and accepts what they paint) |
| `./scripts/test NAME [--update]` | the tests whose full names (module path included, such as `media::tests`) contain NAME, in every crate that has one, with all their output; it finds each crate's `:test` and runs it with `BE3_VERBOSE=1` and `--nocapture`; `--update` accepts their paintings |
| `./scripts/buck test //crates/...` | the tests alone; outside CI it prints only the failures and the compiler's errors, and `BE3_VERBOSE=1` prints everything |
| `./scripts/buck test //crates/editors/checklist:test` | one crate's tests, compared against the accepted paintings |
| `./scripts/buck test //crates/editors/checklist:test -- --env UPDATE_SNAPSHOTS=1` | the same, accepting its paintings |
| `./scripts/buck test //crates/editors/checklist:test -- --test-arg=adding` | only the tests whose names contain `adding` |

### Arguments to `buck test`

Arguments after `--` go to the test executor:

- `--env NAME=VALUE` sets a variable.
- `--test-arg=NAME` passes NAME to the test binary, which runs only the tests matching it.
  Always write it with `=`: the spaced `--test-arg NAME` takes every argument after it as its
  value, a second `--test-arg` included, and the test binary then fails on that.
- A bare argument after `--` is an error.

### Where tests run

- Most tests run on the build server's workers, including beui-renderer-wgpu's, be-dmabuf's,
  be-wayland's and beui-adapter-drm's, which draw through lavapipe from
  `buck/sysroot:amd64-test`.
- A test whose `env` names `LD_LIBRARY_PATH` is built as a harness binary and run through
  `library_path_test`, since the workers replace that variable (`guides/build_server.md`, How
  actions run).
- buck2 runs a test again every time, so `cargo_test` also makes `:test-run`, an action that
  runs the test on a worker and passes when it does, which is cached like any other.
  `./scripts/verify` builds those instead. compile_fail's tests have one too.
- A test with `remote_execution = "disabled"` gets a `:test-run` that runs on this machine.
- buck2 runs a binary's tests on threads, like `cargo test`, so tests that touch process-wide
  state must take turns.

## Plugin tests

A plugin's tests (an editor's, or beui-demo's) run where the plugin runs: compiled to wasm and
started by the same wasmtime host the app opens a plugin with. Nothing about writing one
changes because of that, but what the editor paints is what the shipped plugin paints, with the
FreeType, HarfBuzz and fonts it ships, not what the test machine has installed.

### How they run

1. buck2 compiles the crate's tests for `wasm32-wasip1-threads` against the WASI sysroot the web
   build uses, on the build server.
2. Cranelift compiles each test module once, as a cached action of its own, into a `.cwasm` the
   runner maps in, so a run that changed nothing takes seconds, and a machine that never built
   the plugin gets it from the build server.
3. `crates/plugin-test-runner` runs the module. It is wasmtime with the plugin's own imports
   linked: the gpu abi a plugin draws through, the threads a plugin spawns, and the repository
   itself. It opens a graphics adapter only when a test calls the gpu abi, so a machine without
   one still runs every test that does not, and what a test compares never passes through a
   gpu.

Where they run decides where the paintings go:

- **`./scripts/buck test`** runs a crate's `:test` on this machine, writing accepted paintings
  straight into `snapshots/`.
- **`./scripts/verify`** builds each crate's `:test-run`, an action on a worker that reads the
  accepted paintings as inputs and outputs the paintings that changed and the names of the ones
  it compared. `./scripts/verify` copies the changed ones into `snapshots/` and deletes the
  paintings no test named. A failure's output is in the build's log, not on this machine.

Do not build or test an editor package for the host by itself. Its tests only mean anything as
the wasm guest it ships as, and a native run refuses to compare a painting rather than make one
nothing would agree with.

### When one fails

- A panic aborts a wasm guest, so a failing test ends its binary's run. The runner then lists
  the tests, runs each in an instance of its own, and reports every one that failed at the end
  of the output, each with its panic message and the functions it panicked in, innermost first.
- Rust's backtraces are not available in wasm, so the runner reads the call stack from wasmtime:
  it has function names but no line numbers.
- Until a failure, the module's tests run one after another, in name order, on one thread. What
  a crate keeps per thread (the frame clock, the pinned wall clock) carries from one test into
  the next, so anything that starts afresh with each harness must be reset when it does
  (`HeadlessPlugin::new` restarts the frame clock).
- A test that fails only after others ran passes alone, and in the runner's rerun too.
  Reproduce it by running the whole module with no filter through `./scripts/buck test`, which
  runs it again every time where `./scripts/verify`'s cached `:test-run` does not.

## compile_fail tests

`compile_fail/` holds code that must not compile.

- `compile_fail(name, deps)` in its `BUCK` makes `<name>/lib.rs` a crate and `:<name>-test`.
- The test reads the crate's `[diag.json]` and passes when every line marked
  `//~ ERROR <part of the message>` has that error and no other line has one.
- It is rustc's check pass once for all the cases, with nothing linked.
- The crates sit outside `crates/` because `build //crates/...`, `//:check` and clippy would
  fail on them, and the autofixes would delete the markers.

## Writing a GUI test

GUI tests run headless: no window, no input, no server. A test builds an editor, drives it the
way a person would, and checks what the block became and what the editor painted. They are fast
(well under a second each) and run in `./scripts/verify`.

Tests live inside the editor's crate, one test per file under `src/tests/`.

### What to assert on

- **The block.** An editor turns gestures into operations, so assert on the content the
  operations reached once the test, standing in for the host, applied them.
- **The painting**, as a snapshot: one frame, or a recording of several (see Paintings). This
  catches what an assertion on the block cannot: a control that vanished, a panel that lost its
  contents, a colour that changed.
- **Never** a coordinate, a widget's size, or the order of the accessibility tree. Those change
  whenever anyone touches a layout and say nothing about whether the editor works.

### Test ids

Find widgets by an id the test names, never by their label, so renaming a button, giving it an
icon or moving it to a sidebar leaves the tests alone.

```
<Button label="Add task" @test_id={"checklist.add"} on_click={add} />
```

- `@test_id` names the node the component builds. Its accessibility node carries the id, and
  the frame reports the rectangle of every named node the accessibility tree shows, which is
  what the harness clicks. `drive ids` uses the same ids (`guides/running/drive.md`).
- Name them `<editor>.<what it does>`.
- Where there are many of a kind, key them by whatever the block itself keys them by
  (`checklist.item.{id}.done`, where `id` is the item's `ObjectId`, the same id the edit
  names), never by the order they are drawn in.
- An id names one node in the tree. A copy hidden by a `Show`, a panel a dock is not showing,
  or a closed overlay does not count. Two nodes in the tree with the same id fail the test that
  asks for it, so a control drawn in two places (a toolbar and a dock, say) gives each copy its
  own id.

### The harness: `BeuiTest`

`block_ui_test::BeuiTest` is built from an `OwnedEditor`, which owns the state behind the
`Copy` `Editor` the view is handed. It derefs to one, so `let editor = *owner;` keeps a handle
after the `BeuiTest` takes the owner.

```
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
```

It runs the editor headless through the plugin's own session (`block_editor_plugin::headless`):
the `Screens` and `EditorSession` the shipped plugin runs, fed the protocol messages the app's
host would send, with every message in both directions encoded and decoded as on the wire.

- Gestures become the protocol's input events the way block-app turns beui's, so a key or
  modifier the protocol cannot carry never reaches the editor here either.
- A message the plugin refuses fails the test.

### Content and the block store

There is no server and no block store in an editor test: `BeuiTest` is the host, and does what
the app's host does for the editor's block.

| Method | What it does |
|---|---|
| `hold(block, content)` | gives the harness a block's content: `None` for the editor's own, `Some(id)` for one the editor watches with `content_of` (handed over only once the editor has asked for it) |
| `run()` | paints a frame, applies every operation the editor sent to what it holds, and sends them back as operations marked as the editor's own, until nothing changes; the first content a block gets is a snapshot and everything after it an operation, as in the app |
| `content::<C>(block)` | what a block holds now |
| `edit::<C>(block, &operation)` | an edit arriving from someone else |
| `seeded()` | the content the editor seeded or replaced |
| `store()` | the `ContentStore` behind it, which also stands in for the graph |

On `store()`: `own` and `add_block` put blocks in it, `block` reads one back, `created` lists
the blocks the editor made through its `Blocks` handle, and `defer(true)` holds the editor's
operations back until `defer(false)`, to test what the editor shows before the host has taken
an edit.

### Messages between host and editor

- Whatever else the plugin sends the host is kept for the test: `take_view_changes()`,
  `take_requests()`, `take_opens()`, `take_block_commands()` and the other `take_` methods
  drain one kind, and `sent()` is all of it.
- The host's side goes in the way the app sends it: `set_view`, `set_chrome`,
  `with_top_bar`, `drag_block`, `reply`, `set_histories`, `set_peers`, `report_children`,
  `resize` and `presence_visible` each queue the message the app would send.
- Setting the same state on the `EditorHost` directly skips the protocol, and the session
  overwrites what it owns (the chrome, the view, a drag) on the next frame.

### Frames and gestures

- Call `run()` after every gesture: the view only sees an event on the frame it is delivered in.
- Click a text field before typing into it.
- `click()` panics when no node has that test id, usually a node that was never given one.
  `shown()` and `label()` ask about one without clicking it.

`run()` hands the editor what was queued since the last one in the frames a person would
produce it in, as the app batches input between frames:

- a click (the move to it, the press and the release), a key press or typed text arrives in a
  single frame;
- a drag spans three: the press, the move, and the last move with the release, since beui folds
  a frame's pointer events into one position.

`next_frame()` ends the frame being queued, and `step(events)` paints one frame with exactly the
events it is handed.

### Waiting for work and animations

An editor that hands work to another thread paints a spinner until the work lands, so which of
the two a snapshot captures depends on whether the thread beat the frame: it passes on an idle
machine and fails when the suite runs thirty editors at once. Wait for the work, not for a
number of frames: a few more `run()` calls is the same race with a wider margin.

- `settle_until(description, predicate)` paints until a predicate over the harness holds, and
  fails the test (rather than the painting) if it never does. Between frames it waits as the
  app does: for the editor's `Waker`, or for the delay the editor asked to be painted again
  after, never a fixed time. Work that lands without waking the editor and without asking for
  a frame stalls it until the deadline, which is the stall a person would see in the app.
- `settle()` does the same for the editor's own animations, such as a sheet sliding in: it
  paints until a frame stops asking to be painted again at once. Call it before a `snapshot` or
  `record()` of something that animates in.

```
editor.advance(Duration::from_secs(60));

editor.settle_until("the lighting to land", |editor| editor.shown("scene.lit"));
```

### Clocks

Time in the editor is the harness's clock, not the wall clock. Every frame the harness paints
is one frame interval later, and the host hands the plugin that time with each frame, which is
what its frame clock (`timer::now()`) reads. So an animation lands on the same frame on any
machine, a frame captured part way through one paints the same everywhere, and waiting for a
delay the editor asked for costs no real time.

- `advance(by)` moves the clock on and paints a frame, for a timer longer than an animation.
- `wait(by)` moves the clock on without painting and hands the plugin what the host has queued
  (a `set_host_value`, say), the way a message reaches an idle editor between frames. The next
  `run()` paints the frame that takes it; the editor's effects at the top of that frame read
  that frame's time from `timer::now()`, not the last frame painted.

The date and time of day come from the harness too:

- An editor reads them from `block_editor_beui::wall_clock()` (and `utc_offset()` for local
  time), never `SystemTime::now()`. The harness pins that clock to one fixed moment plus its
  frame clock, so a clock or calendar paints the same on every run.
- beui's own date and time components (a calendar's today, a picker's now) read
  `beui::reactive::wall_clock()`, which the plugin runner pins to that clock each frame.
- A beui test without a plugin pins it with
  `beui::reactive::pin_wall_clock(Some((timer::now(), since_epoch)))`.

### The region: size, scale and viewport

- The region is 800 by 600 points at a scale factor of 1.
- `with_size(Vec2::new(390.0, 800.0))` makes it the size of a phone, and
  `with_scale_factor(2.0)` draws it as a high-density screen does.
- An editor whose manifest claims `pan_and_zoom` draws into a view the host owns, so its test
  calls `in_viewport()`. The harness then does what the host does around the main region: it
  holds a zoom and an offset, hands the editor a view over the content band the editor reported
  (the whole region until it reports one), and answers the pan, zoom and fit the editor asks
  for, fitting the content until the first of them arrives (`guides/plugins/pan_and_zoom.md`).
- An editor not in a viewport is told nothing about a view and fills its region.

### Documents that are not editors: `DocumentTest`

A beui document that is not an editor (beui's demo, say) is painted the same way through
`block_ui_test::DocumentTest`, which drives the document itself with only the fonts beui
carries. Its tests are compiled to wasm with `plugin_tests` like an editor's;
`crates/beui-demo` paints every page of the demo that way. Its clock is a frame clock too:
every `frame()` is one frame interval later, and `advance(by)` moves it on and paints a frame.

## Paintings

### Making one

`editor.snapshot(name)` writes everything the editor painted into
`snapshots/<crate>.<name>.paint`, in one folder at the root of the repository that holds every
painting the workspace accepted.

- A painting holds the rounded rectangles, glyphs and triangles beui's renderer would hand the
  gpu, each glyph carrying its own coverage image, compressed. It is a few kilobytes rather than
  the hundreds a screenshot costs, and it is compared exactly, so it is never flaky.
- A painting is a recording: one frame by default, or the frames the test kept. `record()` keeps
  the frame the editor has just painted, and `snapshot(name)` writes the frames kept since the
  last snapshot, or the current frame if it kept none. Recording a gesture at a time shows how
  the editor got somewhere, not only where it ended up.

```
editor.record();
editor.click("checklist.add");
editor.run();
editor.record();
editor.snapshot("adding_an_item_puts_it_on_the_list");
```

- The frames of a recording share one table of textures, so a frame that draws the same text as
  the last costs only the triangles that draw it.
- Keep recordings to the frames that say something: every frame is compared, so a frame nobody
  looks at is one more way for the test to fail.

### What a painting holds and does not

- **No glyph atlases.** Each glyph carries its own image (coverage, or colour for an emoji from
  a colour font), keyed by its contents, so where a glyph landed in an atlas cannot reach the
  file: text an earlier frame drew (a temporary directory's name, a uuid, the time) repacks
  the atlases without changing the snapshot. Text that varies in the captured frame is a
  different painting, so keep it out of the frame.
- **No `Drawing` contents.** What a beui `Drawing` draws (a plugin's surface, a 3D scene) never
  reaches the painter: the snapshot keeps the region and nothing inside it. A test of one
  asserts on the block instead.
- **Only carried fonts.** Compiled to wasm, beui shapes and rasterizes its glyphs through the
  HarfBuzz and FreeType it carries, with the fonts it carries, so what an editor paints is
  comparable like anything else.

### Out-of-bounds check

`snapshot(name)`, and every `record()` before it, also fails on what the frame shows out of
bounds (beui-core's `Document::overflows`):

- a node cut off by a clip that no scroll or pannable canvas can move it out from under (the
  region's edge, a sidebar too narrow for a row of controls);
- text drawn outside its own box.

Each line names the node, its nearest ancestor with a test id, and the two rectangles. Fix the
layout (wrap or ellipsize the text, let the row shrink with `ItemSize::Intrinsic.shrink(1.0)`,
scroll it), not the test.

### Accepting and reviewing

- `./scripts/verify` runs the tests with `UPDATE_SNAPSHOTS=1`, so a new or changed painting is
  written into `snapshots/` rather than failing the run.
- On a pull request CI does the same, and when that writes a painting it fails the run and
  pushes the painting to the pull request's branch as a commit. Everywhere else CI runs
  `./scripts/ci --check`, which sets nothing, so a painting that was never committed fails.
- `./scripts/verify` fails on a painting a test names with a literal, `snapshot("name")`, that
  `snapshots/` does not hold once it is done: the test failed, or returned, before painting it.
- Once every plugin test passes, `./scripts/verify` deletes each painting in `snapshots/` that
  no test compared, so renaming or removing a snapshot takes its old file with it; with
  `--check` it fails on them instead. A single editor's test run leaves the folder alone.
- Git never merges a painting: when two branches change one, it conflicts. Take either side
  (`git checkout --theirs` or `--ours` on it) and let `./scripts/verify` repaint it.
- Changed paintings are cheap and mechanical to regenerate (a change to beui's renderer
  rewrites every one), so you need not review or explain them, and should leave the images
  alone. A person reviews them on the pull request before merging, in a Paint review block,
  which reads the folder from the repository's dev branch, so a painting is reviewed once it
  has been pushed. Approving is not git and a reviewer is not the tests: a painting nobody
  approved is new again the next time the block is opened, and one nobody had approved before
  it vanished is not reported at all.
- The exception is a painting you did not expect to change: if your change should not have
  touched it, find out why (below).

### Inspecting paintings

- Turn one into a PNG; a trailing frame number or `all` picks the frames of a recording:

  ```
  ./scripts/buck run //crates/paint-snapshot:example-rasterize -- snapshots/<crate>.<name>.paint out.png
  ```

- See what changed: `./scripts/buck run //crates/paint-snapshot:example-changed` says, for every
  painting in `snapshots/` that differs from `HEAD`, which frame changed and what moved in it.
  - `-- --images` also writes a before, an after and a changes image (magenta marks every pixel
    that differs) for every frame that differs, into `target/changed-paintings`, and prints
    which is which. `--images` takes a folder to write to instead.
  - `--base` compares against another revision.
