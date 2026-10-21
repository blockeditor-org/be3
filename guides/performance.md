Measuring and profiling beui's performance, and the ideas already tried.

1. Benchmarks

Benchmarks run in release, on this machine, with `./scripts/buck run -m release`:

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
- `//crates/beui:example-text-bench -- [NAME]` times resizing and editing text.
- `//crates/be-graph:example-access-bench` times the server's access queries (one block,
  what one account sees, a block's audience) on an organization of 100,000 blocks.
- `//crates/editors/infinite-canvas:example-canvas-bench -- [open|pan|zoom|ink]` opens the
  canvas editor through block-ui-test, the way its tests do, on 100 and on 10,000 shapes. It
  pans and zooms by the host's view, and draws pen strokes, and prints each frame's wall
  time (the whole host round trip), beui's layout, paint and interaction, and the most
  nodes one frame measured, placed and painted. For ink it also counts the frames between
  a pen event's delivery and the first frame whose damage covers the pen, which should
  always be 0. `shape()` is the one place it names the canvas model. An editor with an
  `examples/` folder is built for the host as well (`editor()` in buck/wasm/defs.bzl), which
  is how an editor gets a benchmark.

The target for opening a page is under 2 ms warm. The canvas bench measured, in October
2026:

| | 100 shapes | 10,000 shapes |
|---|---|---|
| open | 28 ms | 2.0 s |
| pan at scale 1, p50 wall | 0.06 ms | 0.07 ms (layout 0.01 ms) |
| pan at scale 0.1, p50 wall | 0.08 ms | 0.48 ms (layout 0.30 ms) |
| zoom, p50 wall | 0.16 ms | 0.43 ms (layout 0.24 ms) |
| pen move, p50 wall | 0.05 ms | 0.06 ms |
| pen lift, p50 wall | 0.6 ms | 1.3 ms (interaction 0.9 ms) |
| frames from pen event to ink | 0 | 0 |

A pan, zoom or pen move places and paints the same few nodes whatever the count. A canvas
finds the items near its view through a grid of world cells, so only what is in view costs
anything: at scale 0.1 all 10,000 shapes are in view. A pen lift reads only the new stroke:
the editor keeps its frames, embedded blocks and content bounds in a catalog fed by the
entities each change touched, a keyed mapping that grows at its end builds only the new
rows, and the canvas files only the items that changed into its grid. What is left of a
lift on 10,000 shapes is building the stroke's item and copying the lists of ids, which
is linear but cheap.

Opening still builds an item for every shape. Building them only once they come near the
view would need each item's reach (stroke, hand-drawn wobble, labels and text) before the
item exists, and would move the cost into the first pans over a dense canvas; it would at
best halve opening 10,000 shapes, since loading the document and the test host's store
take as long as building the items.

The test host fits a canvas to its content until the viewer moves it, and `set_view` does
not count as moving it, so the bench sets its view again before each stroke.

2. Comparing two builds

Time on a shared VM drifts by 10% or more between runs, which hides most single changes:

- Build both binaries first, then run them alternately, several times each:
  `./scripts/buck build -m release //crates/beui-demo:example-open-bench --out DIR/base`
  on the base and again with `--out DIR/change`.
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

4. Ideas tried and set aside

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
