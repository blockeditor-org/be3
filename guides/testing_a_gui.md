GUI tests run headless: no window, no input, no server. A test builds an editor, drives it the
way a person would, and checks two things — what the block became, and what the editor
painted. They are fast enough to belong in ./scripts/buck run //:verify: the handful that exist run in
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

@test_id names the node the component builds, and the frame reports the rectangle every
named node was laid out at, which is what block-ui-test clicks. Name them
`<editor>.<what it does>`, and where there are many of a kind, key them by whatever the
block itself keys them by (`checklist.item.{id}.done`, where `id` is the item's `ObjectId`,
the same id the edit names), never by the order they happen to be drawn in.

3. Write the test

Tests live inside the editor's crate, one test per file under src/tests/, like every other
test in the repository. block-ui-test's BeuiTest is built from the same Editor the view is
handed and runs it headless through the plugin's own session (block_editor_plugin::headless):
the Screens and EditorSession the shipped plugin runs, fed the protocol messages the app's
host would send, with every message in both directions encoded and decoded as it would be on
the wire. Gestures are turned into the protocol's input events the way block-app turns
beui's, so a key or a modifier the protocol cannot carry never reaches the editor here
either, and a message the plugin refuses fails the test.

    let block = Uuid::new_v4();
    let host = EditorHost::default();
    host.set_editable(true);
    let editor = Editor::new(host, block);
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
take_requests(), take_opens(), take_block_commands() and the other take_ methods drain one
kind, and sent() is all of it. The host's side of a conversation goes in the same way the
app sends it: set_view, set_chrome, with_top_bar, drag_block, reply, set_histories,
set_peers, report_children, resize and presence_visible each queue the message the app
would send. Setting the same state on the EditorHost directly skips the protocol, and the
session overwrites what it owns - the chrome, the view, a drag - on the next frame.

Call run() after
every gesture — the view only sees an event on the frame it is delivered in — and click a
text field before typing into it. click() panics when no node has that test id, which is
usually a node that was never given one; shown() and label() ask about one without clicking
it.

run() hands the editor what was queued since the last one in the frames a person would
produce it in, as the app batches input between frames: a click (the move to it, the press
and the release), a key press or typed text arrives in a single frame, and a drag spans three
- the press, the move, and the last move with the release - since beui folds a frame's
pointer events into one position. next_frame() ends the frame being queued, and
step(events) paints one frame with exactly the events it is handed.

An editor that hands work to another thread paints a spinner until the work lands, so which
of the two a test captures is down to whether the thread beat it to the frame: a snapshot
that passes on an idle machine and fails when the suite is running thirty editors at once.
settle_until paints until a predicate over the harness holds, and fails the test rather
than the painting if it never does. Between frames it waits the way the app does: for the
editor's Waker, or for the delay the editor asked to be painted again after, and never for a
fixed time. Work that lands without waking the editor and without asking for a frame stalls
it until the deadline, which is the stall a person would see in the app.

    editor.settle_until("the lighting to land", |editor| editor.shown("scene.lit"));

Wait for the work rather than for a number of frames: a few more run() calls is the same
race with a wider margin, which is how one of these hid.

An editor whose manifest claims pan_and_zoom draws into a view the host owns, so its test
calls in_viewport() on the harness. The harness then does what the host does around the main
region: it holds a zoom and an offset, hands the editor a view over its region, and answers
the pan, zoom and fit the editor asks for, fitting the content until the first of them
arrives. An editor that is not in a viewport is told nothing about a view and fills its
region, which is what an editor without that capability does anyway.

The region is 800 by 600 points at a scale factor of 1; with_scale_factor(2.0) draws it the
way a high-density screen does.

4. Snapshots of the painting

editor.snapshot(name) writes everything the editor painted into
snapshots/<crate>.<name>.paint, one folder at the root of the repository holding every
painting the workspace accepted: the rounded rectangles, glyphs and triangles beui's
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

- ./scripts/buck run //:verify accepts whatever the tests paint: it runs them with UPDATE_SNAPSHOTS=1, so
  a new or changed painting is written into snapshots/ rather than failing the run. On a pull
  request CI does the same, and when that writes a painting it fails the run and pushes the
  painting to the pull request's branch as a commit. Everywhere else CI runs ./scripts/buck run //:verify -- --check, which sets nothing,
  so a painting that was never committed fails there.
- Once every plugin test passes, //:verify deletes each painting in snapshots/ that no test
  compared, so renaming or removing a snapshot takes its old file with it; with --check it
  fails on them instead. A single editor's test run leaves the folder alone.
- A changed painting is for a person to review, not for you. They review it in a Paint
  review block, which reads the folder from the repository's dev branch, so a painting is
  reviewed once it has been pushed rather than from the machine that made it. Approving is
  not git and a reviewer is not the tests: a painting nobody approved is new again the next
  time the block is opened, and one nobody had approved before it vanished is not reported
  at all.
- ./scripts/buck run //crates/paint-snapshot:rasterize-example -- snapshots/<crate>.<name>.paint out.png
  turns one into a PNG, and a trailing frame number or all picks which frames of a recording
  to write. It is for a person looking at a painting on the machine that made it; the review
  that matters still happens in a Paint review block.
- Regenerating them is cheap and mechanical - a change to beui's renderer rewrites every one - so a
  changed painting is not by itself a failure to explain, and there is nothing in it for you
  to look at. Say in your handoff which paintings changed and why, and leave the images
  alone.
- The exception is a painting you cannot account for: if you do not know why one changed,
  restore the committed file and run the tests without UPDATE_SNAPSHOTS - git restore
  snapshots/ && ./scripts/buck run //:verify -- --check --plugin-tests - and the failure says which frame
  changed and what moved in it, which is what you needed rather than the image.

A snapshot never holds the glyph atlas. Each glyph carries its own coverage image, keyed by
what is in it, so where a glyph happened to land in the atlas cannot reach the file: text an
earlier frame drew - a temporary directory's name, a uuid, the time - repacks the atlas
without moving anything in the snapshot. Text that varies in the frame the test captures is
of course a different painting, and still has to be kept out of it.

Nor does a snapshot hold what a beui Drawing draws - a plugin's surface, a 3D scene - since
those contents never reach the painter: the snapshot keeps the region and nothing inside it,
so a test of one asserts on the block instead.

beui shapes and rasterizes its glyphs through the HarfBuzz and FreeType it carries, with
the fonts it carries, once it is compiled to wasm, which is how its tests run, so what an
editor paints is comparable like anything else.

5. Running them

./scripts/buck run //:verify runs them, through buck2: ./scripts/buck run //:verify -- --plugin-tests is the plugin
tests alone, and accepts what they paint. buck2 compiles each plugin's tests for
wasm32-wasip1-threads against the WASI sysroot the web build uses, on BuildBuddy's workers,
and runs the module here through crates/plugin-test-runner, which is wasmtime with the
plugin's own imports linked: the gpu abi a plugin draws through, the threads a plugin
spawns, and the repository itself, opened so that a test writes the painting it accepted
into snapshots/. That last part is why a plugin test runs on this machine while the rest of
the tests run on a worker. Do not build or test an editor package for the host by itself:
its tests only mean anything as the wasm guest it ships as, and a native run refuses to
compare a painting rather than making one nothing would agree with.

While working on one editor, run its tests alone:

  ./scripts/buck test //crates/editors/checklist:test
  ./scripts/buck test //crates/editors/checklist:test -- --env UPDATE_SNAPSHOTS=1

The first compares, the second accepts. A panic aborts a wasm guest, so a failing test ends
its binary's run; the runner then lists the tests and runs each in an instance of its own, and
reports every one that failed. A single test is a filter handed through to the
test binary: ./scripts/buck test //crates/editors/checklist:test -- --test-arg some_test_name.

Cranelift compiles each test module once, as an action of its own, and leaves the machine
code as a .cwasm the runner maps in, so a run that changed nothing takes seconds; that
action is cached like any other, so a machine that never built the plugin gets it from
BuildBuddy. The runner only opens a graphics adapter when a test calls the gpu abi, so a
machine without one still runs every test that does not, and what a test compares never
passes through a gpu.
