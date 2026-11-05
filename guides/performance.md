# Performance

Measuring and profiling beui's and be-host's performance. Ideas already tried and set aside are
in `decisions/` ("Ideas set aside", below).

## Benchmarks

`//crates/be-tool-bench:suite` is every benchmark in one directory, built in release whatever
the command asks, with be-host and its apps for the end-to-end ones:

    ./scripts/buck run //crates/be-tool-bench:suite -- list
    ./scripts/buck run //crates/be-tool-bench:suite -- run [--rounds=N] [NAME...]
    ./scripts/buck run //crates/be-tool-bench:suite -- compare BASE [--rounds=N] [NAME...]

`run` prints each metric's fastest, median and slowest over N rounds (3). `BENCHMARKS` in
crates/be-tool-bench/src/suite.rs lists them and says how to add one.

### be-host end to end

`//crates/be-host-bench:bin` (the suite's `app-canvas-and-text`) times what a person waits for:
signing in through the portal, making a folder, a canvas and a text block, drawing a rectangle
and a pen line of 10,000 samples (`HOST_BENCH_POINTS`), pasting 50 MB of text
(`HOST_BENCH_TEXT_BYTES`), saving, closing and exporting; then a second device recovers the
account, reopens both blocks and exports again. It fails unless both exports are identical and
each block paints the same on both devices. It takes about three minutes; lower the two sizes
while working on it.

- It drives be-host headless through `be_tool_drive::Driven` with `BEUI_RENDERER=none`, so it
  measures the host's work and not a software GPU, against a be-server in its own process 15 ms
  away each way (`BE_LATENCY=30`), the server pinned to one core. Every wait is one the host
  answers (`drive wait`, `settle`), never a poll, and every command passes `--changes=0`.
- "Save" is how long `hurry on` takes to settle after an edit shows: the seal a person's edit
  would wait 750 ms for, published to the server.
- The pen stroke (`drive pen draw`) sends its first and last 400 samples at 4 a frame, as a
  stylus does, and the rest at 1,000 a frame, so `canvas.line-draw` measures the work the samples
  cause rather than 10,000 frames. Each slow stretch reports its longest host frame and canvas
  app step (`drive frames`) and how long the ink takes to catch up with the pen (`-catch-up`). A
  host frame waits for apps only until its deadline (guides/apps/runtime.md, Settling), so the canvas's share shows on the app's
  line. The export must hold exactly the samples the canvas keeps (667 of 10,001,
  `canvas.line-points`).
- It does not measure GPU drawing, frame pacing, TLS setup, bandwidth, a large organization, a
  warm relaunch, or two devices editing at once.

### beui

- `//crates/beui-demo:example-open-bench -- [PAGE]` opens each demo page from its catalog row,
  once cold and `REPEAT` (10) times warm in a new document each, and prints cold, warm fastest,
  median and slowest, allocations per open and geometric means. `VERBOSE=1` splits each open
  into interaction, layout, paint and accessibility; `UNPLACED=1` lists what an open built but
  never laid out, by component, which is what to build lazily; `GPU=1` adds the renderer's
  `prepare`, which without a GPU mostly measures lavapipe. The target is under 2 ms warm. Each
  open builds a new document, so per-node tables grow from empty every time; a long-lived
  document reuses slots, so that growth in a profile is not a real cost.
- `//crates/beui-demo:example-space-bench -- [cursor|resize]` (the suite's `space`) runs the demo
  in the VR simulator on an offscreen GPU: `resize` drags panel corners, `cursor` switches
  between two cursor icons every frame. It prints the median and slowest `Runner::frame` and
  `present`, beui's layout and paint, and how many windows' outputs changed. On lavapipe judge
  `frame`, not `present`; `NO_GPU=1` measures only the program's work. In October 2026 a resize
  step's frame was 7 to 9 ms and a cursor switch's about 4 ms.
- `//crates/beui-host:example-text-bench -- [NAME]` times resizing and editing text.
- The open and space benches run the real `Runner` with no drive layer (so no accessibility
  dump) and a stopped clock, pushing inputs as a platform would.

### Apps and the server

- `//crates/be-wasm-host:example-gpu-calls` times an app's per-draw gpu calls through wasmtime
  into wgpu's noop device: about 130 ns a call in October 2026.
- `//crates/be-block-graph:example-access-bench` times the server's access queries on an
  organization of 100,000 blocks.
- An editor's `examples/` are built for the host too (`app()` in buck/wasm/defs.bzl), which is
  how an editor gets a benchmark.

### The canvas

`//crates/be-app-infinite-canvas:example-canvas-bench -- [open|pan|zoom|drag|ink|long] [COUNT]`
opens the canvas through be-sdk-test on 100 and 10,000 shapes and prints each frame's wall time
(the whole host round trip, including the test host applying each edit), beui's phases, and the
most nodes a frame measured, placed, painted and described. For ink it counts frames from a pen
event to the first frame whose damage covers the pen. `shape()` is the one place the bench names
the canvas model. Measured in October 2026 (p50 wall):

| | 100 shapes | 10,000 shapes |
|---|---|---|
| open | 22 ms | 2.0 s |
| pan at scale 1 | 0.06 ms | 0.17 ms |
| pan at scale 0.1 | 0.10 ms | 4.1 ms |
| zoom | 0.11 ms | 1.5 ms |
| drag | 0.17 ms | 0.54 ms |
| pen move | 0.21 ms | 0.42 ms |
| pen lift | 0.13 ms | 0.24 ms |
| long stroke, first and last 200 samples | 0.37, 0.53 ms | 5 to 7 ms each |
| frames from pen event to ink | 0 | 0 |

What keeps a gesture's cost independent of the document's size:

- Every frame of a gesture writes the document (guides/adding_an_app.md, Gestures), but nothing walks it:
  a be-block-model `Document` updates its block references from what each edit touched; the
  host's worker encodes a snapshot only every few hundred operations; and pending edits rebase
  under a peer's by what either touched (`Document::rebase`), with followers told by a `Beneath`
  operation rather than a snapshot (guides/blocks/apps.md).
- A pan, zoom or pen move places and paints only what is in view, found through a grid of world
  cells. A pen lift reads only the new stroke: the editor's catalog of frames, embedded blocks
  and bounds is fed by the entities each change touched, and only changed items are refiled in
  the grid (which the eraser also tests against).
- Accessibility follows the camera: each space a node lays children into is one accessibility
  node whose transform is the space's, so a pan or scroll changes that one transform.
- A pen move costs the same at the end of a long stroke as at its start: the pen appends only new
  points (`InfiniteCanvasOperation::Extend`), the projection rereads only what was appended
  (`Model::reread`), and `freehand::LiveStroke` recomputes only the stroke's end, painting the
  settled part as kept meshes joined on unantialiased seams (`PathPaint::with_seams`).
- Opening still builds an item for every shape (decisions/canvas_builds_every_item_on_open.md).

## Comparing two builds

Time on a shared VM drifts by 10% or more between runs, which hides most single changes. Build
both suites first, then alternate them:

    ./scripts/buck build //crates/be-tool-bench:suite --out DIR/base      # on the base
    ./scripts/buck build //crates/be-tool-bench:suite --out DIR/change    # on the change
    DIR/change/bench compare DIR/base [--rounds=N] [NAME...]

`compare` swaps which suite goes first every round and prints how much the change's fastest
differs. A suite copies everything it runs, so a later checkout does not change it.

- A page's fastest warm open is the steadiest figure; one run's median moves with the drift.
- Allocation counts don't drift (the same build gives the same count within one or two), so a
  change meant to allocate less shows there even when its time is lost in noise. Fewer
  allocations can still be slower (decisions/reactive_scopes_keep_their_simple_paths.md).

## Profiling

Release builds keep frame pointers, so perf walks stacks cheaply and completely:

    apt-get install -y linux-tools-generic
    echo -1 > /proc/sys/kernel/perf_event_paranoid
    cargo install rustfilt

The `perf` on the path refuses a kernel it has no package for; run
`/usr/lib/linux-tools-*/perf` instead. VMs have no hardware counters, so it samples the CPU
clock:

    perf record -F 10000 -g -o perf.data DIR/change [PAGE]
    perf report -i perf.data --no-inline --children --sort symbol --stdio -g none \
        -p 'Bench7release' -x --percent-limit 1 | rustfilt
    perf script -i perf.data --no-inline -F ip,sym | rustfilt

`-p` matches the mangled name of the function the measured work runs under, kept out of line
for this (`release` in the open bench, `lift` for the canvas bench's pen lift).

- Always pass `--no-inline`: without it perf runs addr2line on every frame, and a few seconds of
  samples take over fifteen minutes to resolve.
- Don't use `--call-graph dwarf`: it drops samples and cuts off beui's deep reactive stacks,
  skewing the profile towards shallow work.
- Allocation sites: `perf probe -x /lib/x86_64-linux-gnu/libc.so.6 malloc`, record with
  `-e probe_libc:malloc -g`, then `perf probe -d 'probe_libc:*'`.

### Profiling be-host

- `BE3_PERF_MAP=1 ./scripts/buck run -m release //crates/be-host:dev` makes wasmtime write
  `/tmp/perf-PID.map`, naming the apps' compiled wasm; record with `perf record -g -p PID` around
  the `drive` commands being measured.
- Each app runs on a thread named `app <id>` (cut to 15 characters), so `perf report --sort comm`
  splits by app and `--comms 'app be3.data'` keeps one. The block store is `be-host-be`, and
  `:dev`'s server runs on `tokio-rt-worker`. Without a GPU, llvmpipe's threads take most of a
  scroll's samples; leave them out.
- Time waits as well as work: an action that uses little CPU but takes long is waiting on round
  trips to the server. Run `drive hurry off` first, or sealing happens at once and anything shown
  only after a seal looks faster than in a window.
- To fill an organization with data, make a company organization on the dev host, add an agent
  (host menu, Agents), share a workspace with it, and point a `be` built from the checkout at it
  (guides/tracker.md): `be new-database`, `add-column` and `import` of a generated CSV make a
  tracker of any size.

## Starting be-host

Reopening the host goes straight to the workspace the last session left, with every step that
can overlap running at once (decisions/host_startup_overlaps_app_loading.md):

- The last organization is kept with its account in the host's state, so a restart opens it
  without waiting for the server's organization list (`resume_organization` in
  crates/be-host/src/main.rs), falling back to the list if it is gone.
- The apps whose editors were open, the shell's app and the apps its manifest `opens` are warmed
  as soon as the host knows it will open a workspace, before its window and GPU exist
  (`warm_apps`, `be_host_runtime::warm`): a warmed app loads and links on its own thread and
  waits for the device only when it first reaches the GPU. Linking its libraries (about 0.3 s
  each) is most of an app's start; mapping its `.cwasm` takes milliseconds.
- An app with no editors open is stopped after `IDLE_TIMEOUT` (30 s, `Host::stop_idle`).
- On desktop every `.cwasm` is mapped from its file (`Module::deserialize_file`) rather than
  read and copied; they total about 400 MB.
- The content of an editor's references (up to four) is prefetched once its block's node is
  known. The be worker joins queued `Open`s concurrently, but awaits listings between commands,
  so a query making several listing requests holds up everything behind it: a block's
  references come in one `ListReferences`.
- On the web, app manifests are fetched together before the first frame. workspace-ui keeps
  tab titles in the view state, so a restored tab is named before its block loads.

## Memory

On Linux under `--desktop`, be-host samples its own memory every 10 s (`crates/be-host/src/memory.rs`)
and keeps a point a minute, the peak of each measurement through it, for 48 hours. The
Performance host panel draws them above its timings; `$XDG_STATE_HOME/be-host/memory.tsv` (beside
the login session's log, and kept as `memory.previous.tsv` by the next run) has the same minutes
as lines of KiB, each wasm guest's memory too, for plotting elsewhere.

- Resident is `RssAnon + RssFile + RssShmem` from `/proc/self/status`. File-backed includes the
  `.cwasm` pages apps have run, which are mapped from their files. Shared is mostly the `wl_shm`
  buffers of Wayland programs' windows, which their own processes count too.
- An app's wasm memory never shrinks, so it is that app's peak heap
  (`be_host_runtime::wasm_memories`, kept by be-wasm-host after each call into the guest).
- `malloc heap, free` is what glibc holds and has not given back (`mallinfo2`). If it grows
  while nothing leaks, the allocator is fragmenting, not leaking.
- A leak shows as a line that keeps rising while the computer is idle, or that steps up at each
  screen lock or terminal switch and never comes back down.

## Size

What ships is `./scripts/deploy`'s `-m release -c be3.lto=true //crates/be-host:native`. In
October 2026 on Linux x86_64 that was 896 MB (297 MB gzipped): `.cwasm` 595 MB (Cranelift's code
is about 2.5× the wasm it compiles), `.wasm` 198 MB (mostly apps, each instantiating beui's and
`beui-reactive`'s generics for its own types; type modules about 1.8 MB each), be-host 102 MB.

To see where it goes: `size -A` and `nm -S --size-sort -C` for be-host, twiggy (`twiggy top -n
100000 --format csv`) for a module, `size -A` on a `.cwasm` (an ELF file). Attribute a generic
instance to the innermost first-party type in its name, not to `core` or `alloc`. Code in a
generic function that does not depend on its type parameter is copied for every instance and
every closure type, so moving it out (`probe_erased`, `Computation::new`'s `parts`) shrinks every
module. The release settings and the alternatives measured are in
decisions/shipped_bundle_size_settings.md.

## Ideas set aside

Each helped but works around a cost rather than removing it; kept for when nothing better is
left: decisions/page_teardown_runs_during_the_frame.md,
decisions/system_allocator_over_jemalloc_or_mimalloc.md,
decisions/accessibility_tree_always_built.md, decisions/no_idle_time_glyph_prerasterizing.md.
Measured and not worth their code: decisions/reactive_scopes_keep_their_simple_paths.md.
