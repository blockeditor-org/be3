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

The target for opening a page is under 2 ms warm.

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

    apt-get install -y linux-tools-generic   # perf is /usr/lib/linux-tools-*/perf
    echo -1 > /proc/sys/kernel/perf_event_paranoid
    cargo install rustfilt

Record the binary `buck build --out` gave you. VMs have no hardware counters, so perf
samples the CPU clock:

    perf record -F 10000 -g -o perf.data DIR/change [PAGE]

Then either report, matching the mangled name of the function the measured work runs
under (the open bench's is `release`, kept out of line for this):

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
  `plugin <id>`, so `--comm` splits the report by plugin. Without a GPU, wgpu draws through
  llvmpipe on its own threads, which take most of a scroll's or resize's samples; leave
  them out when judging the app's own work.
- To fill a workspace with data, add an agent from the dev app's menu (Agents) and point a
  `be` built from the checkout at it with the `BE3_*` values the dialog shows
  (guides/tracker.md): `be new-database`, `add-column` and `import` a generated CSV make
  a tracker of any size.

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
