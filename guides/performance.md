Measuring and profiling beui's performance, and the ideas already tried.

1. Benchmarks

`//crates/bench:suite` is every benchmark in one directory, built in release whatever the
command asks: the runner `bench`, each benchmark program under `programs/`, and block-app
with its plugins for the end-to-end ones. `./scripts/buck run //crates/bench:suite -- list`
lists them; `-- run [--rounds=N] [NAME...]` runs the benchmark each NAME names, or else those
whose name contains it (all by default), N times (3) and prints each metric's fastest, median and slowest. A benchmark is
a program that reports its metrics through the `bench` crate (`bench::time`, `count`,
`record`), which appends them to the file `BENCH_METRICS` names and does nothing without
it; adding one is a line in `BENCHMARKS` (crates/bench/src/suite.rs) and an entry in the
suite's `programs` (crates/bench/BUCK). Each can also run alone, with
`./scripts/buck run -m release`:

- `//crates/app-bench:bin` (the suite's `app-canvas-and-text`) runs block-app headless,
  driven as `drive` drives it (crates/be-drive's `Driven`, guides/running/drive.md), against
  a be-server in its own process, with `BE_LATENCY=30` (guides/running/drive.md), so every
  round trip takes 30 ms more, and `BEUI_RENDERER=none`, so it measures the app's own work and
  not a software GPU drawing it (guides/running/drive.md). It signs a first device in through the portal, as a person's
  browser would, makes a folder and zooms the files into it, then with `hurry off` adds a
  canvas and a text block from the files' New menu, draws a rectangle and a pen line of
  `APP_BENCH_POINTS` samples (10,000), pastes `APP_BENCH_TEXT_BYTES` of text (50 MB), paints
  and closes each, exports the folder as a .block file and closes the app. A second device,
  with data of its own, signs in, unlocks with the recovery phrase, reopens both blocks,
  paints them and exports the folder again. It fails unless both exports are identical and
  each block paints the same on both devices. "Save" is how long `hurry on` takes to settle
  after the edit shows: the seal the app would wait 750 ms for, published to the server.
- The pen line goes in 1,000 samples a frame (`drive pen draw FROM TO STEPS PER_FRAME`), so
  `canvas.line-draw`, from its first sample until the whole line shows, is the work the
  samples cause (the canvas's edits, relaying them, the replica, the plugin applying them)
  and not the cost of 10,000 frames, which a person drawing never waits on.
  `canvas.line-longest-frame` and `canvas.line-longest-plugin-frame` are the longest host
  frame and the longest canvas plugin step meanwhile (`drive frames`, guides/running/drive.md):
  a frame packed with samples is where a per-sample cost that grows with the stroke shows as
  stutter, and since a host frame waits at most 8 ms for its plugins, the plugin's own line
  is where the canvas's share shows. The run fails unless the exported stroke holds exactly
  the samples the canvas keeps, where they were drawn: it keeps a sample only when it is more
  than a point from the last one it kept (667 of these 10,001, `canvas.line-points`),
  so dropping one it would have kept moves the points after it.
  It takes about three minutes; set the two sizes lower while working on it. It needs
  `BLOCK_APP`, and `APP_LIBRARIES` for the libraries the app loads, which the suite sets.

- `//crates/beui-demo:example-open-bench -- [PAGE]` opens each beui-demo page from its
  catalog row, once first in the process (cold) and then `REPEAT` more times (10 by default)
  in a new document each time (warm). It prints each page's cold time, the fastest, median
  and slowest warm time, the allocations one open makes, and a geometric mean of each
  column. `VERBOSE=1` prints every open, split into interaction, layout, paint and
  accessibility; `UNPLACED=1` lists what each open built but never laid out, by the
  components it is in, which is what to build lazily; `GPU=1` adds the renderer's
  `prepare`, which on a machine without a GPU mostly measures lavapipe and says little
  about a real one.
- The open bench builds a new document for each open, so the per-node tables a document
  keeps grow from empty every time. A long-lived document reuses the slots of the nodes
  it removed; don't chase the growth of those tables in a profile of the bench.
- `//crates/beui-demo:example-space-bench -- [cursor|resize]` (the suite's `space`) runs the
  demo in the VR simulator (`beui_layer_space::Space` as the runner's host) on an offscreen
  GPU, through the platform pixels a mouse sends, in each mode (both by default) on a runner
  of its own. For each scene it prints the median and slowest frame (`Runner::frame`, which
  runs the compositor's drawing, and `present`), beui's layout and paint, and how many
  windows' outputs changed (`VERBOSE=1` prints every frame). `NO_GPU=1` leaves out the GPU
  and so measures the app's own work. On lavapipe `present` is mostly the software
  rasterizer; judge `frame`.
  - `resize` floats the Docking tab into a window of its own, and drags a corner of the
    main panel and of that window for 60 frames each. In October 2026 a resize step's
    `frame` was 7 to 9 ms, after 90 to 100 ms while each window built a new `Renderer`
    (pipelines, glyph atlases) for every new size.
  - `cursor` finds two points of the main panel whose cursor icons differ, and moves the
    pointer between them every frame for 60 frames, after 60 frames of moving it a pixel
    over one icon. In October 2026 a switch's `frame` was about 4 ms, as the moves' are,
    after 32 ms while the cursor sprite built a new `Renderer` for every icon.
- `//crates/beui-host:example-text-bench -- [NAME]` times resizing and editing text.
- Both beui benches run their frames through the real `Runner`, with no drive layer (so no
  accessibility dump) and the clock stopped, and push their inputs as a platform would. A
  frame's time is `Runner::frame`'s, so it includes the intake and the window bookkeeping
  around the app's update; the open bench takes the renderer's `prepare` out of it.
- `//crates/block-wasm-host:example-gpu-calls` times a plugin's per-draw gpu calls (set
  pipeline, viewport and scissor, draw) through wasmtime and block-gpu-host into wgpu's
  noop device: about 130 ns a call on this machine in October 2026.
- `//crates/be-graph:example-access-bench` times the server's access queries (one block,
  what one account sees, a block's audience) on an organization of 100,000 blocks.
- `//crates/editors/infinite-canvas:example-canvas-bench -- [open|pan|zoom|drag|ink|long] [COUNT]`
  opens the canvas editor through block-ui-test, the way its tests do, on 100 and on 10,000 shapes
  (or only COUNT). It pans and zooms by the host's view, drags a shape back and forth, draws pen
  strokes, and draws one stroke of 2,000 samples (`long`, timing its first and last 200), and
  prints each frame's wall time (the whole host round trip, including the test host applying
  each edit, keeping its undo history and answering the canvas's references query), beui's
  layout, paint, interaction and accessibility, and the most nodes one frame measured, placed,
  painted, described for accessibility and walked for it. For ink it also counts the frames between
  a pen event's delivery and the first frame whose damage covers the pen, which should
  always be 0. `shape()` is the one place it names the canvas model. An editor with an
  `examples/` folder is built for the host as well (`editor()` in buck/wasm/defs.bzl), which
  is how an editor gets a benchmark.
- `./scripts/buck run //crates/bench:suite -- [run|compare BASE|list] [NAME...]` runs the
  benchmarks crates/bench/src/suite.rs lists, in release, three rounds each (`--rounds=N`).
  `compare BASE` alternates them with a suite built at BASE
  (`./scripts/buck build //crates/bench:suite --out BASE` on the base) and prints each
  metric's fastest and median from both.
- The suite's `app-canvas-and-text` (crates/app-bench) drives block-app headless with no GPU
  (`BEUI_RENDERER=none`) against a be-server in a process of its own, 15 ms away each way
  through `BE_LATENCY`; the server is pinned to one core and the app to the others.
  It times what a person waits for: signing in, making, saving, closing, exporting and
  reopening blocks, and a second device recovering the account. Every wait is one the app
  answers (`drive wait`, `gone`, `settle`), never a poll, and every command passes
  `--changes=0`. Its pen stroke sends 400 samples at the start and 400 at the end at 4 a
  frame, as a stylus does, and the 9,200 between at 1,000 a frame to get there; each of the
  two stretches reports its longest frame and how long the ink takes to reach the pen after
  its last sample (`-catch-up`). It does not measure drawing on a GPU, frame pacing,
  connection and TLS set-up, bandwidth (`BE_LATENCY` delays but does not throttle), a large
  organization, a relaunch on warm caches, or two devices editing at once.

The target for opening a page is under 2 ms warm. The canvas bench measured, in October
2026:

| | 100 shapes | 10,000 shapes |
|---|---|---|
| open | 22 ms | 2.0 s |
| pan at scale 1, p50 wall | 0.06 ms | 0.17 ms (layout 0.05 ms) |
| pan at scale 0.1, p50 wall | 0.10 ms | 4.1 ms (layout 1.4 ms, paint 1.9 ms, accessibility 0.5 ms) |
| zoom, p50 wall | 0.11 ms | 1.5 ms (layout 0.55 ms, paint 0.53 ms) |
| drag, p50 wall | 0.17 ms (interaction 0.09 ms) | 0.54 ms (interaction 0.31 ms) |
| pen move, p50 wall | 0.21 ms (interaction 0.12 ms) | 0.42 ms (interaction 0.22 ms) |
| pen lift, p50 wall | 0.13 ms | 0.24 ms (interaction 0.13 ms) |
| long stroke, p50 wall of its first and last 200 samples | 0.37 ms, 0.53 ms | 5 to 7 ms, 5 to 7 ms |
| frames from pen event to ink | 0 | 0 |

`camera()` centres a world point in the canvas: a view is where the content sits on
screen, not the world rectangle it shows.

Every frame of a gesture writes the document (guides/adding_a_plugin_editor.md), so a drag
or a pen move is an edit the host applies and the editor reads back. None of that walks the
document: a be-model `Document` keeps its block references up to date from what each edit
touched once they have been asked for (the test host asks after every edit, the app's
worker on every publish), the app's worker publishes a block's operations and encodes a
snapshot only every few hundred of them (`Content` folds the operations since its snapshot
when something reads it), and an editor holding edits the host has not acknowledged, or a
follower holding edits the owner has not accepted, rebases them under a peer's edit by what
either touched (`Document::rebase`) instead of rebuilding; the follower's app tells its
editors with a `Beneath` operation rather than a snapshot (guides/the_new_block_stack.md).

Accessibility follows the camera the way placement does. Each space a node lays children
into (a canvas's clipped, unclipped and scaled spaces, a scroll's content) is one
accessibility node whose transform is the space's translation and scale, and the nodes in it
are placed in the space's own coordinates, so a pan or scroll changes that one node's
transform and sends nothing else. A pan at scale 0.1 still walks the canvas's shown items
(cheaply, from what each held) when items come into view, and describes only those. The test
host's accessibility dump renders its text and test id rects only when something reads them,
so the bench reads the canvas's rect once rather than every frame.

A pen move costs the same at the end of a long stroke as at its start. The pen writes only
the new points (`InfiniteCanvasOperation::Extend`, a `Series` append, a stroke keeping its points
as drawn in a `points_frame`), the projection re-reads only what was appended
(`Model::reread`), the block-ref index re-walks only the changed field, and the canvas draws a
stroke being drawn with `freehand::LiveStroke`: it keeps the settled outline, recomputes only
the end, and paints the settled part as pieces whose meshes are kept (`Painter::paint_path`),
joined on seams that are not antialiased (`PathPaint::with_seams`) so they paint as one fill.

A pan, zoom or pen move places and paints only what is in view. A canvas
finds the items near its view through a grid of world cells, so only what is in view costs
anything. A pen lift reads only the new stroke:
the editor keeps its frames, embedded blocks and content bounds in a catalog fed by the
entities each change touched, a keyed mapping that grows at its end builds only the new
rows, and the canvas files only the items that changed into its grid. The same catalog
files each shape's reach into a grid of world cells, which is what the eraser tests.

Opening still builds an item for every shape. Building them only once they come near the
view would need each item's reach (stroke, hand-drawn wobble, labels and text) before the
item exists, and would move the cost into the first pans over a dense canvas; it would at
best halve opening 10,000 shapes, since loading the document and the test host's store
take as long as building the items.

2. Comparing two builds

Time on a shared VM drifts by 10% or more between runs, which hides most single changes:

- Build the suite of each build first, then run them alternately, several times each:
  `./scripts/buck build //crates/bench:suite --out DIR/base` on the base and again with
  `--out DIR/change`, then `DIR/change/bench compare DIR/base [--rounds=N] [NAME...]`, which
  runs each benchmark from one suite and then the other, swapping which goes first every
  round, and prints both suites' fastest and median of each metric and how much the
  change's fastest differs. A suite copies everything it runs, so a later build or
  checkout does not change it.
- Compare each page's fastest warm open, never one run's median.
- Allocation counts don't drift: the same build gives the same count to within one or two.
  A change meant to allocate less shows up there even when its time is lost in noise. They
  are not time, though: reserving room for eight cleanups in every scope saved a
  tenth of the allocations and made opening a page 2.5% slower.

3. Profiling

Release builds keep frame pointers, so perf walks stacks cheaply and completely. Set up:

    apt-get install -y linux-tools-generic
    echo -1 > /proc/sys/kernel/perf_event_paranoid
    cargo install rustfilt

The `perf` on the path is a wrapper that refuses a kernel it has no package for ("perf not
found for kernel"); run the binary it wraps, `/usr/lib/linux-tools-*/perf`, instead.

Record the binary `buck build --out` gave you. VMs have no hardware counters, so perf
samples the CPU clock:

    perf record -F 10000 -g -o perf.data DIR/change [PAGE]

Then either report, matching the mangled name of the function the measured work runs
under (the open bench's is `release` and the canvas bench's pen lift is `lift`, kept out of
line for this):

    perf report -i perf.data --no-inline --children --sort symbol --stdio -g none \
        -p 'Bench7release' -x --percent-limit 1 | rustfilt

or dump stacks to fold them yourself:

    perf script -i perf.data --no-inline -F ip,sym | rustfilt

- Always pass `--no-inline`. Without it perf runs addr2line on every frame, and a few
  seconds of samples take over fifteen minutes to resolve.
- Don't use `--call-graph dwarf`. It copies a fixed slice of the stack per sample, drops
  samples it can't keep up with, and cuts off beui's deep reactive stacks, so the profile it
  gives leans towards shallow work. One that lost samples showed tearing a page down as 43%
  of opening it; frame pointers showed 18%.
- To see where allocations come from, probe malloc:
  `perf probe -x /lib/x86_64-linux-gnu/libc.so.6 malloc`, record with
  `-e probe_libc:malloc -g`, and `perf probe -d 'probe_libc:*'` afterwards.
- To profile block-app itself, start it with `BE3_PERF_MAP=1` (`BE3_PERF_MAP=1 ./scripts/buck run
  -m release //crates/block-app:dev`), which makes wasmtime write `/tmp/perf-PID.map` so the
  plugins' compiled wasm has names, and record the running app with `perf record -g -p PID`
  around the `drive` commands being measured. Each plugin runs on a thread named
  `plugin <id>` (cut to 15 characters, such as `plugin be3.data`), so `perf report --sort comm`
  splits the report by plugin and `--comms 'plugin be3.data'` keeps one plugin's samples.
  Without a GPU, wgpu draws through llvmpipe on its own threads, which take most of a
  scroll's or resize's samples; leave them out when judging the app's own work.
  The block store's worker is the thread `block-app-be`, and `:dev`'s embedded server runs on
  `tokio-rt-worker` threads.
- Time block-app's waits as well as its work: an import or a create that uses little CPU but
  takes long is waiting on round trips to the server, one after another. Run `drive hurry off`
  first (guides/running/drive.md): otherwise the app seals edits at once, and anything shown
  only once an edit is sealed looks faster under `drive` than in a window.
- To fill an organization with data, create a company organization on the dev app's
  Organizations screen (the personal one has no agents), add an agent from its app menu
  (Agents), share a workspace with it, and point a `be` built from the checkout at it with
  the `BE3_*` values the dialog shows (guides/tracker.md): `be new-database`, `add-column`
  and `import` a generated CSV make a tracker of any size.

4. Starting block-app

Reopening the app should go straight from the page or window to the workspace the last
session left, with every step that can overlap running at once. What keeps it that way:

- The last organization opened is kept with its account in app state, so a restart opens
  it without waiting for the server's organization list and keys, which are fetched in
  the background (`resume_organization` in crates/block-app/src/main.rs). If the list
  no longer has it, the app falls back to the organization list.
- The plugins whose editors were open are kept in app state too, and are started with
  the shell's plugin and the plugins of the editors the shell's manifest `opens`
  (`warm_plugins` in crates/block-app/src/main.rs, `block_app_plugin_host::warm`), before the
  shell asks for them. The app warms them as soon as it knows it will open a workspace (an
  account signed in or signing in), before its window and GPU are set up: a plugin warmed
  before `install` loads, links and instantiates on its own thread at once, waits for the
  host's device only when it first reaches the GPU, and becomes the runtime that `install`
  creates for it. Starting workspace-ui and the file tree together, there, took the time to
  the Files pane from about 0.92 s to 0.76 s locally (October 2026). Most of a plugin's
  start is `Plugin::new` linking its libraries (about 0.3 s each, the two at once);
  deserializing its mapped `.cwasm` takes a few milliseconds. A plugin with no editors open
  idles once it has loaded.
- On desktop every `.cwasm` is mapped from its file (`Stored::File`, wasmtime's
  `Module::deserialize_file`) rather than read into memory and copied: the app's
  plugins, libraries and type modules come to about 400 MB, and mapping them took about a
  third of a second off the time to the workspace's Files pane (October 2026).
- The host prefetches the content of an editor's references (up to four) as soon as its
  block's node is known, so a database view's database loads beside the view.
- The be worker joins every queued `Open` concurrently. Its loop still awaits listings
  between commands, so keep each query to one listing request: a block's references
  come in one `ListReferences`, not one `ReadBlock` each.
- On the web, plugin manifests are fetched together, before the first frame.
- workspace-ui keeps each tab's title in the view state's `titles`, so a restored tab is
  named before its block's info arrives.

5. Size

What ships is `./scripts/deploy`'s `-m release -c be3.lto=true //crates/block-app:native`:
the app, every plugin and type module, the two libraries (guides/plugin_libraries.md), and a
`.cwasm` of each. In October 2026 on Linux x86_64 that was 896 MB, 297 MB gzipped:

- `.cwasm`, 595 MB (198 MB gzipped): Cranelift's machine code is about two and a half times
  the wasm code it compiles, so it shrinks only as the wasm code does. Shipping without
  it moves 95 seconds of compiling (four cores) to the first start, 42 of them the beui
  library's.
- `.wasm`, 198 MB: 151 MB of code, of which plugins are most; each instantiates beui's and
  `reactive`'s generics for its own types, which no library can hold. Type modules are
  standalone, 1.8 MB each, mostly `be-model`, `sequence` and `block-type-guest` again.
- block-app, 102 MB: 66 MB of code, 24 MB of symbols, the rest unwind tables and data.

To see where it goes: `size -A` and `nm -S --size-sort -C` for the app, twiggy
(`cargo install twiggy`, `twiggy top -n 100000 --format csv`) for a module, and
`size -A` on a `.cwasm`, which is an ELF file. Attribute a generic instance to the
innermost first-party type in its name, not to `core` or `alloc`: that is what put
`reactive` first. Keep code that does not depend on a type parameter out of generic
functions (`probe_erased`, `Computation::new`'s `parts`), since every closure type copies it.

What is already in place: fat LTO with one codegen unit for the deploy only (`be3.lto`, a ten
minute link), legacy symbol mangling in release (buck/platforms/profile.bzl) and the hashed
export names of linked modules (guides/plugin_libraries.md). Thin LTO left the app as it was,
and `opt-level=s` for wasm took a twentieth off the plugins while growing both libraries and
beui-gallery by four fifths. Stripping the app's symbols or the modules' name sections
would take 24 MB and 27 MB more, but the panic guard's reports and a guest's backtrace read them.

6. Ideas tried and set aside

Each of these helped, but works around a cost rather than removing it. Keep them for when
nothing better is left.

- Disposing a closed page after the frame instead of during it. Teardown should be fast
  rather than hidden; deferring it saved about 12% of opening a page.
- Replacing the system allocator with jemalloc (about 15% faster) or mimalloc (about 6%).
  Allocate less instead.
- Building the accessibility tree only while a screen reader asks for it. It is wanted
  often; make building it cheaper instead. Switching it off saved about 4%.
- Rasterizing ASCII in idle time so a page's first open finds its glyphs ready.

These were measured and changed nothing worth their code:

- Skipping the reactive context a disposing scope enters when disposal is already
  untracked: checking cost more than it saved.
- Giving a component no scope of its own until something is created in it: only about one
  component scope in ten ends up empty, because nearly every component probes a binding.
