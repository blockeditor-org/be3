# Performance

Measuring and profiling beui's performance: the benchmarks, comparing two builds, the
inspector's Perf tab, profiling with perf, and ideas already tried and set aside. Read it before
working on a speedup, and read the last section before trying one of those ideas.

## Benchmarks

Benchmarks run in release, on this machine, with `./scripts/buck run -m release`.

### Opening pages: `example-open-bench`

```
./scripts/buck run -m release //crates/beui-demo:example-open-bench -- [PAGE]
```

It opens each beui-demo page from its catalog row, once first in the process (cold) and then
`REPEAT` more times (10 by default) in a new document each time (warm). It prints each page's
cold time, the fastest, median and slowest warm time, the allocations one open makes, and a
geometric mean of each column. PAGE picks one page.

| Variable | Effect |
|---|---|
| `REPEAT=N` | the number of warm opens (10 by default) |
| `VERBOSE=1` | prints every open, split into interaction, layout, paint and accessibility |
| `UNPLACED=1` | lists what each open built but never laid out, by the components it is in: what to build lazily |
| `GPU=1` | adds the renderer's `prepare`, which on a machine without a GPU mostly measures lavapipe and says little about a real GPU |

The target for opening a page is under 2 ms warm.

The bench builds a new document for each open, so the per-node tables a document keeps grow
from empty every time. A long-lived document reuses the slots of the nodes it removed, so don't
chase the growth of those tables in a profile of the bench.

### Text: `example-text-bench`

```
./scripts/buck run -m release //crates/beui:example-text-bench -- [NAME] [--frames N]
```

Per-frame times for resizing wrapped text and labels, editing a paragraph or one span, and
typing into a text area. NAME picks the scenarios whose names contain it.

## Comparing two builds

Time on a shared VM drifts by 10% or more between runs, which hides most single changes.

1. Build both binaries first:
   `./scripts/buck build -m release //crates/beui-demo:example-open-bench --out DIR/base` on
   the base, and again with `--out DIR/change` on the change.
2. Run them alternately, several times each.
3. Compare each page's fastest warm open, never one run's median.

Allocation counts don't drift: the same build gives the same count to within one or two, so a
change meant to allocate less shows up there even when its time is lost in noise. They are not
time, though: reserving room for eight cleanups in every scope saved a tenth of the
allocations and made opening a page 2.5% slower.

## Timing the running app: the inspector's Perf tab

To time an interaction in a running program rather than a bench, run a release build headless
and read the inspector's Perf tab (the inspector is in `guides/beui/apps.md`):

1. `./scripts/buck run -m release //crates/beui-demo:dev` (or block-app's `:dev`), then drive it
   (`guides/running/drive.md`).
2. Open the tab: `drive key ctrl+shift+i`, then `drive click '"Perf"'`.
3. Act, then read the Peak column from `drive tree`. Reset samples starts over.

- The tab times each frame of the document. Interaction covers whatever the input ran, so a
  component built in response to a click is counted there, not in Layout.
- Take the median of several runs: the headless renderer draws on the CPU and competes for the
  same cores.

## Profiling with perf

Release builds keep frame pointers (`-Cforce-frame-pointers=yes`), so perf walks stacks
cheaply and completely.

### Setting up

```
apt-get install -y linux-tools-generic   # perf is /usr/lib/linux-tools-*/perf
echo -1 > /proc/sys/kernel/perf_event_paranoid
cargo install rustfilt
```

### Recording

VMs have no hardware counters, so perf samples the CPU clock. Record the binary
`buck build --out` gave you:

```
perf record -F 10000 -g -o perf.data DIR/change [PAGE]
```

Or attach to a running `:dev` program, such as beui-demo's:

```
perf record -F 10000 -g -p "$(pgrep -x demo)" -o perf.data
```

To record only part of the time, start perf paused with `-D -1 --control=fifo:ctl,ack` and
write `enable` and `disable` to the `ctl` fifo around the `drive` commands to measure.

### Reading

Report, matching the mangled name of the function the measured work runs under (the open
bench's is `release`, kept out of line for this):

```
perf report -i perf.data --no-inline --children --sort symbol --stdio -g none \
    -p 'Bench7release' -x --percent-limit 1 | rustfilt
```

Or dump stacks to fold them yourself:

```
perf script -i perf.data --no-inline -F ip,sym | rustfilt
```

- Always pass `--no-inline`. Without it perf runs addr2line on every frame, and a few seconds of
  samples take over fifteen minutes to resolve.
- Don't use `--call-graph dwarf`. It copies a fixed slice of the stack per sample, drops samples
  it can't keep up with, and cuts off beui's deep reactive stacks, so the profile leans towards
  shallow work. One that lost samples showed tearing a page down as 43% of opening it; frame
  pointers showed 18%.
- The headless renderer's shaders show up as `[JIT]`; leave them out of a beui profile.

### Where allocations come from

Probe malloc:

1. `perf probe -x /lib/x86_64-linux-gnu/libc.so.6 malloc`
2. Record with `-e probe_libc:malloc -g`.
3. `perf probe -d 'probe_libc:*'` afterwards.

## Ideas tried and set aside

Each of these helped, but works around a cost rather than removing it. Keep them for when
nothing better is left.

| Idea | Gain | Instead |
|---|---|---|
| Disposing a closed page after the frame instead of during it | about 12% of opening a page | make teardown fast rather than hidden |
| Replacing the system allocator with jemalloc or mimalloc | about 15% / about 6% faster | allocate less |
| Building the accessibility tree only while a screen reader asks for it | about 4% | it is wanted often; make building it cheaper |
| Rasterizing ASCII in idle time so a page's first open finds its glyphs ready | not recorded | |

These were measured and changed nothing worth their code:

- Unsubscribing a disposed computation lazily, compacting a signal's subscribers once half are
  stale, instead of `retain`ing the list each time: subscriber lists are short.
- Skipping the reactive context a disposing scope enters when disposal is already untracked:
  checking cost more than it saved.
- Giving a component no scope of its own until something is created in it: only about one
  component scope in ten ends up empty, because nearly every component probes a binding.
