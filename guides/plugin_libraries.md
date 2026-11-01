# Plugin libraries

A plugin is either standalone, one plain module with everything it uses in it, written against
the raw plugin interface below in any language, or linked: a position-independent module that
names the libraries it needs, which the host links it to when it loads it, on wasmtime and in the
browser alike, from the same files. A library holds what plugins have in common (std, beui, wgpu,
FreeType, HarfBuzz and the rest), so a linked plugin's module holds only its own code and the
generics instantiated for its own types, and any number of plugins share one copy of each library,
one file and one compiled module.

## What is in it

A Rust plugin is linked against two libraries, or one (`buck/wasm/defs.bzl`):
- **base**, `crates/plugin-library-base`, the root: std, alloc, the allocator and the panic
  runtime, wasi-libc, libc++ and libc++abi, setjmp, wgpu with its custom backend
  (block-gpu-guest), be-ops, block-plugin-api, block-editor-plugin (with the framework-neutral
  region driver, `block_editor_plugin::driver`, and editor state), geometry, serde, postcard,
  serde_json and uuid, and what they depend on. Nothing in it depends on `reactive` or beui:
  block-editor-plugin's state (content, the graph, host values, and the editor, artifact and
  creation state) is read as plain values and announces changes through callbacks
  (guides/the_new_block_stack.md, "State and change callbacks"), which any ui framework adapts.
- **beui**, `crates/plugin-library-beui`, above base: `reactive`, beui and everything under it,
  block-editor-beui, which turns those callbacks into beui's signals, and beui-platform-plugin
  (the runtime block-editor-beui reaches through a symbol rather than a dependency, which brings
  beui-host: the renderer, fonts and inspector; it is beui's half of the region driver), with
  FreeType and HarfBuzz.

Every editor is linked against both unless its `editor()` says otherwise (`DEFAULT_LIBRARIES`);
an editor with no ui framework, such as triangle, names base alone (`BASE_LIBRARY`). Split base
further only when a module needs part of it, such as a type module that wants std without wgpu.

A library is linked whole and exports everything, since it cannot know what a plugin it has not
seen will use. A crate only one or two plugins use stays in those plugins. To move one into a
library, add it to the library crate's `Cargo.toml` (and its `lib.rs`) and run
`./scripts/buck run //:buckify`.

libc++ and libc++abi are linked whole into the root too (`WHOLE_LIBRARIES` in `crates/wasm-link/src/lib.rs`).
wasi-libc is linked for what the root uses, and the C math library and the libc functions in
`LIBC` are forced in (`MATH` and `LIBC` there), among them what FreeType and PDFium call. A
module that calls a C function no library has fails to link, with the function named; add it
there. A
library none of them links, such as one of wasi-libc's emulations (`libwasi-emulated-mman.a`),
stays in the plugin's own link.

A C or C++ library a linked module links has to be compiled position-independent, as everything
the plugins' toolchain compiles is (`_wasi_flags` in `buck/toolchains/BUCK` gives a plugin's C
`-fPIC`, which cc-rs leaves out on wasm, so a build script's C is too): a shared module cannot
take any other code. Build it from source on that toolchain, as third-party/pdfium builds PDFium,
rather than linking an archive built elsewhere. The C++ runtime and setjmp are base's even though
nothing in base uses them, so every C++ library above it shares one copy (`crates/plugin-library-base/BUCK`).

## Identity

A library is one exact build: Rust has no stable ABI, so a module works only with the libraries it
was linked against. A library's identity is the SHA-256 of its module. A linked module names every
library in its closure, in dependency order, with exported globals `__be3_lib_<position>_<identity>`
(`wasm_dylink::libraries` reads them, which survive precompiling), so one plugin's module pins its
whole closure. A library is staged beside the plugins as `lib-<identity>.wasm` (and `.cwasm` where
plugins are precompiled; `buck/app/stage.sh`), every library in the closure of every staged module:
each module's rule gives its closure as `WasmLibrariesInfo` (`buck/wasm/libraries.bzl`), which the
`app` rule stages. Both hosts hash a library's `.wasm` before using it and refuse one whose SHA-256
is not the name it was found by. A `.cwasm` cannot be checked that way and is trusted as it is
found.

Every linked module, a library or a plugin, carries a `be3.lib` custom section, lines of text the
links read: `name` (the module's), `need` (each library in its closure by identity, in dependency
order), `link` (each Rust archive and C library it holds) and `tls` (each of its thread-locals).

## How the links are made

Every wasm link runs through `crates/wasm-link` (the toolchain's linker in `buck/tools`), which
otherwise passes the link to rust-lld unchanged. The rules are in `buck/wasm/defs.bzl`.

- **The root** (`wasm_library(module)` with no `libraries`, `--be3-library=NAME`): every Rust
  object linked whole and everything exported (`--export-all`), with a growable exported function
  table, and wasi-libc's `crt1-reactor.o`, whose `_initialize` sets up libc's main thread and runs
  the constructors. It is linked at fixed addresses (below).
- **A library above the root** (`wasm_library(module, libraries)`, `--be3-library=NAME` and a
  `--be3-needs=MODULE` for each library): position-independent (`-shared`), every Rust object linked
  whole and everything exported.
- **A linked plugin** (`editor(libraries)`, `linked_plugin(module, libraries)`: `--be3-needs`
  alone): position-independent, its own code only.

`libraries` names every library in the module's closure, not only the ones it uses directly. A
link that needs libraries reads their `be3.lib` sections and:
- refuses a library that needs one the link does not name, and orders the rest so each comes
  after the ones it needs (`closure`);
- refuses a closure in which two libraries hold the same crate or C library, naming it (two copies
  of `reactive` would be two reactive runtimes, which fails silently);
- leaves out of the link every archive and C library the closure holds, so what the module uses of
  them becomes imports;
- reaches the thread-locals they define through `GOT.tls` (below);
- checks every import of the linked module against the closure (`unresolved`): a symbol no library
  exports fails, naming it and the importing module, and so does one more than one library defines,
  apart from what lld defines in every module (`SYNTHESIZED`);
- renames every mangled symbol it imports or exports (`_R`, `_Z` and LLVM's `anon.`) to `#` and
  sixteen characters of its SHA-256 (`wasm_dylink::short_symbol`), as the root's link does its
  exports: a library exports every symbol it has, and their names were a third of its module.
  The loaders only ever match names, so nothing else changes; the `name` section keeps the readable
  names for backtraces, and `unresolved` and the `be3.lib` listing name symbols as they were;
- adds the `__be3_lib_*` globals, and `__be3_dylink_<memory size>_<memory align>_<table size>_<table
  align>_<tls size>_<tls align>`, which a host reads from the compiled module before
  instantiating it.

A package whose crate other crates depend on, such as `crates/tls-diamond`'s, has a `library.rs`
beside its `Cargo.toml` that only names the crate, and `wasm_library` links that, so the module
holds the very rlib its dependents compile against: Buck's separate cdylib compile of the crate
would have another crate hash, and its symbols would not match.

Every crate on the plugins' platform is already compiled position-independent (buck2's default),
so nothing else changes about how they build.

`editor(libraries = [])` links an editor standalone instead (`editor_module` in
`buck/wasm/defs.bzl`, which also builds a second module of one editor, such as triangle's
`:standalone`): every crate it uses linked into one plain module, no `--be3-*` flag, a WASI reactor
as the root is. lld places the main thread's thread-locals itself in a module that is not
position-independent. block-app-plugin-host's
`a_standalone_triangle_paints_what_the_triangle_linked_against_its_library_paints` loads triangle
both ways on a real device and compares the pixels.

### The root

One module stays the root of every linked closure: the one holding std and wasi-libc. Rust's
prebuilt std for wasm32-wasip1-threads and both wasi-libc builds for it (Rust's self-contained one
and `third-party/wasi`'s) are compiled with absolute addresses (`R_WASM_MEMORY_ADDR_SLEB`), which
rust-lld refuses in a `-shared` link; only wasm32-wasip1's wasi-libc has a position-independent
`libc.so`. So the root is linked at fixed addresses, and brings its own static data, stack,
`__stack_pointer`, heap (`__heap_base` to the memory's end, then `memory.grow`), table and
main-thread thread-local block. The loader owns everything above it: it creates the memory from the
root's import, places every other module's static data in pages it grows the memory by, gives each
module its table slots, and gives each module above the root a thread-local block on every thread.
Making the root position-independent too needs std built from source (`-Zbuild-std`) and wasi-libc
built with `-fPIC` for threads.

## Thread-locals

LLVM compiles a thread-local access as an offset from the module's own `__tls_base`
(local-exec) on every target but Emscripten, whatever model the IR asks for: `global.get
__tls_base`, `i32.const` of the symbol with an `R_WASM_MEMORY_ADDR_TLS_SLEB` relocation, `i32.add`.
That offset cannot point into another module, and rust-lld refuses the relocation against an
undefined symbol. A plugin reaches a library's thread-locals through the generics it
instantiates, so every module has its own block on every thread and reaches another module's
thread-locals through a global the loader fills, as ELF's dynamic TLS model does:
- **The link.** A link that needs libraries scans its inputs, objects and the members of archives
  (`inputs.rs`, `archive.rs`), for TLS relocations against symbols nothing in the link defines, and
  rewrites those objects before rust-lld (`tls.rs`): the `i32.const` becomes a `global.get` of the
  same size, its relocation an `R_WASM_GLOBAL_INDEX_LEB`, and the undefined data symbol an
  undefined global imported as `GOT.tls.<symbol>` (mutable i32). An archive it changes is written
  again with its symbol table's offsets moved. A relocation with an addend, or one outside the
  code, is refused; LLVM makes neither for position-independent code.
- **The value.** The code still adds its own `__tls_base`, so `GOT.tls.<symbol>` holds the
  symbol's address on this thread less the importing module's `__tls_base` on this thread:
  `thread_local_slot` in `crates/wasm-dylink`. A module exports its thread-locals' offsets in its
  block as globals (rust-lld does, with `--export-all`), so the address is the defining module's
  `__tls_base` on this thread plus that offset. The root's block on a spawned thread is wasi-libc's,
  read from the thread's start argument, since the root sets its `__tls_base` only once
  `wasi_thread_start` runs.
- **The blocks.** The plan lays every module above the root out in one run of blocks per thread
  (`Blocks`: the size and alignment of the run, and each module's offset in it, from the
  `__tls_size` and `__tls_align` the link wrote into its layout export). The loader carves runs
  from pages it grows the memory by, takes one for the main thread and one for each thread a
  plugin spawns, in `wasi.thread-spawn` on the spawning thread before the thread runs, and gives it
  back when the thread's `wasi_thread_start` returns. Natively the free runs are a list in the
  spawner (`Linked` in `block-wasm-host/src/dylink.rs`); in a browser they are a list in the memory
  itself, under a lock at the plan's `region`, since any worker may spawn a thread or end one
  (`takeBlocks` and `freeBlocks` in `block-app/web/dylink.js`).

`crates/tls-diamond` holds the test of it: base as the root, left and right as libraries above it,
and a plugin needing all three (`linked_plugin`) whose generics reach a thread-local in each, on
the main thread and on a spawned one, which wakes it to send `[1]` once all of them were right.
block-wasm-host's `the_thread_local_diamond_runs_on_every_thread` loads it through `Host` on
wasmtime, and `//crates/tls-diamond:web-test` boots it through `plugin.js` in Chromium, where a
second worker checks that a library which no longer hashes to its name is refused.

## Loading

`crates/wasm-dylink` holds the names the link and the loaders agree on, and plans the link as
plain data (`plan`, a `Plan`): for each module in the list, root first and plugin last, where its
static data goes, its table slots, what answers each of its imports (a function or tag of a module
below it, an address in a module at or below it, a table slot, a thread-local of a module below
it, or the host), and where its thread-local block lies in a thread's run. A standalone plugin is
a list of one, with nothing to plan. `crates/block-wasm-host/src/dylink.rs` (wasmtime) and
`crates/block-app/web/dylink.js` (the browser, which reaches the planner through block-gpu-shim)
only carry it out:
1. Instantiate the root against the memory its import asks for, shared, and run `_initialize`.
2. Plan: grow the memory by the pages the other modules' static data needs (`pages`) and place it
   there, where the root's allocator never reaches (wasi-libc's `sbrk` grows from wherever the
   memory ends, and dlmalloc takes what it gets as a new segment), and pick table slots after the
   root's own for each module's functions and for every function whose address one takes.
3. Link the rest in order (`link`), each one: instantiate it, answering its `env` imports with the
   exports of the modules below it, `GOT.mem` with addresses and `GOT.func` with table slots; fill
   the table slots of the functions it exports; on the main thread, apply its data relocations;
   `__wasm_init_tls` with its block, then fill its `GOT.tls`; on the main thread, run its
   constructors. Each module's constructors so run before anything above it is instantiated.

A thread the plugin spawns instantiates the root and links the whole list again from the same
plan, with the blocks taken for it. Everything is linked when the plugin starts, before any
thread exists, since a table cannot be shared between threads: there is no loading of another
module later.

Natively the host keeps one compiled module per library identity per engine (`Host` in
block-wasm-host; one wasmtime engine per cache directory), so every plugin after the first only
compiles itself. `Host::load_from` and `PureHost::load_from` take a reader that hands back each
module as a `Stored`: a file (`Stored::file`), whose `.cwasm` wasmtime maps rather than copies,
or bytes already in memory, as Android's assets are. block-app-stack's `discovery::open` is the
app's reader: a file beside the executable on desktop, an asset's bytes on Android. In a browser each plugin's worker fetches each library and compiles it with
`WebAssembly.compileStreaming`, which lets the browser cache the compiled code for the next worker
that fetches the same file; the server has to give `.wasm` the `application/wasm` type for that,
and `fetchLibrary` compiles the bytes it hashed otherwise.

## The raw plugin interface

A plugin needs no Rust and no library: a standalone plugin is one plain wasm module, with
no `dylink.0` section and no `__be3_lib_*` export, which the host instantiates as it is, on
wasmtime (`Program::Whole` in block-wasm-host) and in the browser (`boot` in
`block-app/web/plugin.js`). `crates/block-wasm-host/src/tests/standalone.wat` is one, in
WebAssembly text, that paints through the gpu; block-wasm-host's tests run it on wasmtime and
`//crates/block-wasm-host:standalone-web-test` in Chromium.

Every guest, an editor plugin or a module with no GPU, speaks the same small ABI, in
`crates/block-guest-abi`. What a standalone plugin exports:
- `plugin_start`, `plugin_step` and `plugin_shutdown`, with no parameters or results. Required.
  The host calls `plugin_start` once, `plugin_step` whenever frames wait or the plugin woke, and
  `plugin_shutdown` as it stops it.
- `memory`: the memory every pointer it passes points into, unless it imports one. Not shared,
  unless it spawns threads.
- `_initialize`, the WASI reactor's: called once after instantiation, before anything else.
  Without it the host calls `__wasm_call_ctors` if it is exported, which is what LLVM's
  constructors are run by. Optional.
- Its world's sync exports, each `(has_parent: i32, parent: i64, pointer, length) -> i64` (the
  result's pointer and length packed), with `plugin_alloc(length) -> pointer` for the host to
  write the arguments into (guides/type_modules.md, "Operations and WIT"). Only those its world
  has.
- `wasi_thread_start(thread, argument)`, if it imports `wasi.thread-spawn`.

What it may import, each only if it uses it (anything else is answered by a function that
fails when called):
- From `be3_host`: `host_send(pointer, length)` and `host_receive(pointer, capacity) -> i64`,
  which carry the frames of its world (postcard, as in guides/type_modules.md; `NO_MESSAGE`
  when nothing waits, and the length needed when the buffer is too small), `host_wake()`, which
  asks to be stepped again, and `host_panic(pointer, length)`, which stops it with that message.
- Its world's sync imports, `(has_parent: i32, parent: i64, pointer, length)` with no result,
  under the interface's name (an editor's, `crates/block-plugin-api/wit`).
- The `be3:gpu/device@0.1.0` imports (below): one per func, resources as `i32` handles that start
  at 1 for each kind, a record of plain values flattened into parameters, and anything else as
  the pointer and length of its postcard encoding; `gpu_local_imports!` in block-gpu-abi lists
  every import and its parameters.
- One memory, under any name, shared or not, in place of exporting one.
- `wasi_snapshot_preview1`, for a plugin built for WASI.
- `wasi.thread-spawn(argument) -> i32`, for a plugin with threads, which then has to import a
  shared memory: each thread instantiates the module again against it and calls
  `wasi_thread_start`. A thread may only call `host_wake` and `host_panic`.

A guest written in Rust implements `block_guest_abi::Guest` (`start` and `step`, with the frames
in and out as plain bytes) and `block_guest_abi::guest!(Type)` exports it; the panic hook, the
receive buffer and the wake flag are the crate's. block-editor-plugin's `plugin!` is one such
guest, plus the gpu, WASI and threads an editor has.

A linked plugin follows the linked-module convention on top of this, which belongs to the link,
not to the interface: it and its libraries import one shared `env.memory` (the root's import, with
its limits), and the exports and imports under "How the links are made" and "Loading".

### The gpu

An editor's wgpu calls are the `be3:gpu` world (`crates/block-gpu-abi/wit`, shaped after
wasi:webgpu), which be-ops-bindgen generates with `local` (guides/type_modules.md,
"Operations and WIT"): its imports, module `be3:gpu/device@0.1.0`, are answered on the
plugin's own thread, so they return their results, and a call made per draw takes only
wasm values. block-gpu-guest is wgpu's custom backend over the guest bindings (a
surface is the host's surface id; `release_surface` drops it), block-gpu-host's `Gpu`
implements `Host` over a real wgpu device, block-wasm-host's `Guest` (`State` with a
view of the plugin's memory, shared or its own) is the `Caller` and links the glue, and block-gpu-shim is the `Caller` in a browser worker and exports
each import as `module#name`, which `plugin.js` binds every import it does not answer
itself to. A call whose arguments cannot be read is refused into the device's error,
which `take-error` and the step both report; natively a plugin that reaches for a gpu
no device could be opened for stops with why. `example-gpu-calls` in block-wasm-host
times the per-draw calls (guides/performance.md).

The plugin GPU API is all of wgpu that browser WebGPU exposes, core and optional
features, with the optional features and limits the host's device has. Anything missing
from that set is a gap, and its `unimplemented!` in `block-gpu-guest/src/objects.rs`
says "not yet". Plugins are untrusted, so whatever wgpu marks `unsafe` is refused for
good: passthrough shaders and pipeline caches. Native-only extensions (mesh shaders, ray
tracing, multi-draw-indirect with counts) are added only behind a feature the plugin
checks, since the same plugin runs in a browser; until then their messages, and those
of immediate data and `clear_texture`, say they are not in browser WebGPU.

`features` and `limits` in the WIT are the host device's: its optional features that
browser WebGPU has (`convert::FEATURES` in block-gpu-host, which leaves out immediate
data), and its WebGPU limits. The app's device asks its adapter for those features
(`browser_features` in beui-renderer-wgpu), as the shim's device does in a browser.
`block_gpu_guest::features()` and `limits()` read them, and a plugin gets its device
from `block_gpu_guest::request_device`, which refuses features and limits beyond them
as wgpu's `request_device` does; block-editor-plugin requests every offered limit and no
feature. The device then has only what it requested, and `features.rs` in the guest
refuses, as a validation error, a texture format, storage format, blend or
`unclipped_depth` that needs a feature it did not request, and a shader whose `enable`
names one, and a timestamp query set without `timestamp-query`. The host refuses the rest
against its own device. A bind group gains the same check (`float32-filterable`) when it
learns its view formats. `indirect-first-instance` cannot be checked: the first instance
of an indirect draw is in a gpu buffer, so a plugin that did not request it gets what the
host's device does with a nonzero one (honoured wherever the device has the feature).

Buffer mapping is asynchronous on both hosts, as in a browser. `map_async` asks the host
(`buffer.map-async`), which maps the buffer and wakes the plugin once the map settles:
natively a thread of the `Gpu`'s own polls the device while a map is pending, and the
waker is the guest's `Wake` (block-wasm-host's `State::gpu`); in a browser the map settles
on the worker's event loop and the shim calls the wake `plugin.js` binds with `bind_wake`.
The guest answers the map's callback only when it polls: `device.poll` (which waits for
the gpu natively and cannot in a browser) or `block_gpu_guest::poll`, which
block-editor-plugin calls at the start of every step. It asks `buffer.map-state`, WebGPU's
`mapState`, of each pending map. `get_mapped_range` copies a range once out of a map
(`buffer.read-mapped`, whose bytes come back raw), except for a buffer mapped at
creation, which starts zeroed; a range written through `get_mapped_range_mut` goes back
when it is dropped (`buffer.write-mapped`). Unmapping or dropping a buffer fails its
pending map. The host refuses, as an error rather than a panic, a read or write outside
the mapped range, misaligned, or a write to a read map. Copies between buffers and
textures, `clear_buffer`, compute pipelines and passes (`dispatch_workgroups` and
`dispatch_workgroups_indirect`), and implicit bind group layouts (`layout: None` and
`get_bind_group_layout`, on render and compute pipelines) are recorded as wgpu records
them. A compute pass's calls, like a render pass's, take only wasm values. A device without
compute (the WebGL fallback) reports zero compute workgroups per dimension in its limits
and refuses a compute pipeline as a validation error. Textures stay
on the gpu; only a mapped read crosses into the plugin's memory. Render and compute
pipelines pass their stages' overridable `constants` (keyed by name or `@id`);
`zero_initialize_workgroup_memory` is not WebGPU's and the host always zeroes. A texture
view's `usage` narrows its texture's. `destroy` frees a buffer (failing its pending map) or
a texture the plugin created; a surface's or imported texture is the host's, and
destroying it does nothing.

The guest's other asynchronous answers settle the way maps do: the host wakes the plugin,
and the guest calls back when it polls. `on_submitted_work_done` takes a ticket from the host
(`on-submitted-work-done`), which asks its queue and polls in the background like a map, and
the guest asks `submitted-work-done` of each ticket. `get_compilation_info` is a future that asks
`shader-module.get-compilation-info`, which polls wgpu's future on the host with a waker that
wakes the plugin (natively the answer is there at once; in a browser it comes from a promise),
and a pending future is woken on the guest's next poll. The host keeps a module whose WGSL was
rejected, as WebGPU does, so its messages say where the WGSL went wrong; one the guest refused
answers its refusal. Device loss is per plugin: `Device::destroy` destroys only the plugin's
view (`destroy-device`), never the host's shared device: the host destroys the buffers and
textures the plugin made, fails its pending maps, and from then on drops creates (answering
handles to nothing), writes, submits and maps, and reports no errors, until the plugin
requests a device again (`request-device`, which `block_gpu_guest::request_device` calls).
`device-lost` answers why the device was lost, and the guest's poll calls the lost callback
with it. A device lost by the host reaches the plugin only where the plugin is the device's
only user: the shim's `Gpu::owning_device` takes the device's lost callback. Natively the app
shares its device, keeps its own lost callback, and restarts every plugin on its new device
(`replace_gpu`). Debug markers and groups reach the host on encoders and passes; a pass's take
the label as the pointer and length of its UTF-8 in the plugin's memory (a `string` parameter
of a `local` func), so they stay wasm values. wgpu has no debug markers on render bundle
encoders (and wgpu-core refuses them in bundles), so a plugin cannot ask for them.

Occlusion and timestamp query sets, `resolve_query_set`, a render pass's
`occlusion_query_set` and occlusion queries, render and compute passes' `timestamp_writes`, render bundles
(`create_render_bundle_encoder`, `execute_bundles`) and `draw_indirect` and
`draw_indexed_indirect` are WebGPU's. Timestamps inside encoders and passes
(`write_timestamp`) and pipeline statistics are native-only and not in browser WebGPU.
A resolved timestamp is in nanoseconds, coarsened to 100 microseconds as Chrome does, on
both hosts: the host gives a buffer made with `QUERY_RESOLVE` the `STORAGE` usage too
(when its device has `timestamp-query`), and after resolving timestamps into it runs a
compute pass (`queries.rs` in block-gpu-host) that converts the device's ticks
(`get_timestamp_period`) to nanoseconds and rounds them down, so the guest's period is
1. The host records a render bundle's commands itself and replays them into wgpu's
encoder at `finish` (`bundles.rs`), since wgpu's encoder borrows what it is given. Two
of wgpu's gaps are filled by an overlay of the wgpu crate (`third-party/rust/overlays/wgpu`,
see `fixups.bzl`): a custom backend's bundle encoder can be finished (`finish_custom`),
a bundle that fails validation goes to the device's error scope instead of panicking
the host, and a render pass's occlusion query set reaches the browser's pass; refresh or
drop it when wgpu is upgraded. On WebGL, indirect draws are refused (wgpu's
`INDIRECT_EXECUTION` downlevel flag), which a plugin cannot ask about beforehand.

A *pure guest* has only those imports: no gpu, no library, no WASI, no threads, no shared memory, no
clock. It is a `wasm32_module` (wasm32-unknown-unknown) rather than an `editor()`, and exports its
own memory. `crates/block-guest-echo` is the smallest one, which the tests start every way. A
type module is a pure guest whose ABI grows with its WIT world (guides/type_modules.md,
"Operations and WIT"): its async calls are be-ops frames, and each sync func is a wasm
import (`(has_parent: i32, parent: i64, pointer, length)`, with no result) or export
(the same arguments, the host writing them into memory from `plugin_alloc`, returning
the result's packed pointer and length). The host links `be3_host` and the world's imports (`PureHost::with_imports`,
`WebGuest::start`'s list) and refuses any other.

The host half is `crates/block-guest-host`, which the editor host and anything else that runs a
guest share:
- `Worker<C: Channel>` queues frames, steps the guest once it is ready, carries what it sent and
  finds a step that runs past `STEP_TIMEOUT`. It steps only when frames are waiting or the guest
  woke (`Channel::take_woken`), so an idle guest costs nothing to poll; a guest that holds
  output back for a later step wakes itself.
- `thread::spawn` runs a `thread::Runnable` on a thread of its own and is the `Channel` for it;
  its loader is handed the `Wake` to give the guest, which marks the guest woken before it
  reaches the host. Dropping it interrupts the guest. Editors (`block-app-plugin-host/src/wasm.rs`) and pure guests both use it.
- `InProcess<G: Guest>` runs a guest's Rust in the same process, for unit tests.
- `web::WebGuest` runs a pure guest in a web worker of its own, with frames moved as
  transferable buffers and nothing else in the worker: no gpu shim, no dylink, no
  SharedArrayBuffer.

Natively `PureHost` in block-wasm-host loads a pure guest with an engine of its own
configuration (threads, shared memory and relaxed SIMD off, epoch interruption on), links only
the `be3_host` imports so a guest that imports anything else is refused, and caps its memory with
`StoreLimits` (`MEMORY_LIMIT`, or `with_memory_limit`). `PureHost::spawn` puts it on its own
thread. It reads a `.cwasm` beside the module as `Host` does, and `precompile` compiles either
kind.
