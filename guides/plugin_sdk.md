# The plugin SDK

Every editor is linked against one shared module, the plugin SDK (`crates/block-plugin-sdk`),
instead of carrying its own copy of std, beui, wgpu, FreeType, HarfBuzz and the rest. A plugin
module holds only its own code and the generics instantiated for its own types; the host links
it to the SDK when it loads it, on wasmtime and in the browser alike, from the same files. A
plugin may instead be standalone, one module with everything it uses in it, written against the
raw plugin interface below in any language.

## What is in it

`crates/block-plugin-sdk` is a crate that depends on what the plugins have in common: beui,
block-editor-beui, beui-platform-plugin (the runtime block-editor-beui reaches through a symbol
rather than a dependency, which brings beui-host: the renderer, fonts and inspector; it is beui's half of the region driver), block-editor-plugin (with the framework-neutral half, `block_editor_plugin::driver`), serde, serde_json and uuid, and through them
everything underneath. block-editor-plugin itself does not depend on `reactive` or beui: its
state (content, the graph, host values) is read as plain values and announces changes through
callbacks, and block-editor-beui turns those into beui's signals (guides/the_new_block_stack.md,
"State and change callbacks"). All of it is linked into the SDK whole and exported, since the SDK cannot
know what a plugin it has not seen will use. A crate only one or two plugins use stays in those
plugins. To move one into the SDK, add it to the crate's `Cargo.toml` (and its `lib.rs`) and
run `./scripts/buck run //:buckify`.

The SDK's Rust is linked whole, and so are libc++ and libc++abi (`WHOLE_LIBRARIES` in
`crates/wasm-link/src/lib.rs`), since the SDK cannot know what C++ a plugin will use either.
The other C libraries under it (wasi-libc, libsetjmp) are linked for what the SDK uses, and
the C math library and the libc functions in `LIBC` are forced in (`MATH` and `LIBC` there). A
plugin that calls a C function the SDK does not have fails to link, with the function named;
add it there. A library the SDK never links, such as one of wasi-libc's emulations
(`libwasi-emulated-mman.a`), stays in the plugin's own link.

A C or C++ library a plugin links has to be compiled position-independent, as everything the
plugins' toolchain compiles is: a shared module cannot take any other code. Build it from
source on that toolchain, as third-party/pdfium builds PDFium, rather than linking an archive
built elsewhere.

## Identity

An SDK is one exact build: Rust has no stable ABI, so a plugin works only with the SDK it was
linked against. The SDK's identity is the SHA-256 of its module. A plugin says which it needs
with an exported global named `__be3_sdk_<identity>`, and the SDK is staged beside the plugins as
`sdk-<identity>.wasm` (and `.cwasm` where plugins are precompiled; `buck/app/stage.sh`), so any
number of plugins built against one SDK share one file and one compiled module.

Both hosts hash an SDK's `.wasm` before using it and refuse one whose SHA-256 is not the name it
was found by. A `.cwasm` cannot be checked that way and is trusted as it is found.

## How the links are made

Every wasm link runs through `crates/wasm-link` (the toolchain's linker in `buck/tools`), which
otherwise passes the link to rust-lld unchanged.

- `--be3-sdk` (the SDK's `linker_flags`, from `plugin_sdk()` in `buck/wasm/defs.bzl`): links
  every Rust object whole and exports everything (`--export-all`), with a growable exported
  function table. It writes a `be3.sdk` custom section listing the archives and libraries it
  linked and the SDK's thread-locals.
- `--be3-plugin=SDK` (each editor's `linker_flags`): links the plugin as a position-independent
  shared module (`-shared`), leaving out every archive the SDK linked, so what the plugin uses of
  them becomes imports. It then checks that the SDK exports every import, and adds the identity
  and an export `__be3_dylink_<memory size>_<memory align>_<table size>_<table align>`, which a
  host reads from the compiled module before instantiating it.

Every crate on the plugins' platform is already compiled position-independent (buck2's default),
so nothing else changes about how they build.

`editor(libraries = [])` links an editor standalone instead (`editor_module` in
`buck/wasm/defs.bzl`, which also builds a second module of one editor, such as triangle's
`:standalone`): every crate it uses linked into one plain module, no `--be3-*` flag, with
wasi-libc's `crt1-reactor.o` so it is a WASI reactor whose `_initialize` sets up libc's main
thread (`__wasi_init_tp`) and runs the constructors. lld places the main thread's thread-locals
itself in a module that is not position-independent, so it needs no `plugin_initialize_tls`.
block-app-plugin-host's `a_standalone_triangle_paints_what_the_triangle_linked_against_the_sdk_paints`
loads triangle both ways on a real device and compares the pixels.

## Thread-locals

LLVM compiles a thread-local access as an offset from the module's own `__tls_base`
(local-exec) on every target but Emscripten, whatever model the IR asks for: `global.get
__tls_base`, `i32.const` of the symbol with an `R_WASM_MEMORY_ADDR_TLS_SLEB` relocation, `i32.add`.
That offset cannot point into another module, and rust-lld refuses the relocation against an
undefined symbol.

Today a plugin reaches the SDK's thread-locals through generics it instantiates, so the two share
one thread-local block:
- The SDK's link ends its thread-locals with a reserve, `__be3_plugin_tls`
  (`PLUGIN_TLS_SIZE`, 16 KiB), in every thread's block.
- A plugin's link starts its thread-locals with a generated object that defines each of the
  SDK's at the SDK's offset, so the plugin's own come after them, inside the reserve.
- The host points the plugin's `__tls_base` at the SDK's block for each thread and copies the
  plugin's own initial values into the reserve. The link fails if a plugin's own thread-locals
  outgrow it.

That works only with one module above the SDK. The design plugins move to
(plans/plugin_libraries.md) gives every module its own block on every thread and reaches another
module's thread-locals through a global the loader fills, as ELF's dynamic TLS model does:
- **The link.** A link with `--be3-needs=MODULE` (`Mode::Linked` in `crates/wasm-link`) scans
  its inputs, objects and the members of archives (`inputs.rs`, `archive.rs`), for TLS relocations
  against symbols nothing in the link defines, and rewrites those objects before rust-lld
  (`tls.rs`): the `i32.const` becomes a `global.get` of the same size, its relocation an
  `R_WASM_GLOBAL_INDEX_LEB`, and the undefined data symbol an undefined global imported as
  `GOT.tls.<symbol>` (mutable i32). An archive it changes is written again with its symbol table's
  offsets moved. A relocation with an addend, or one outside the code, is refused; LLVM makes
  neither for position-independent code.
- **The value.** The code still adds its own `__tls_base`, so `GOT.tls.<symbol>` holds the
  symbol's address on this thread less the importing module's `__tls_base` on this thread:
  `thread_local_slot` in `crates/wasm-dylink`. A module exports its thread-locals' offsets in its
  block as globals (rust-lld does, with `--export-all`), so the address is the defining module's
  `__tls_base` on this thread plus that offset.
- **The loader.** Every thread instantiates every module, and globals are per instance, so the
  loader fills each module's `GOT.tls` globals once per thread, after `__wasm_init_tls(block)` has
  set its `__tls_base` and before any of its code runs: `thread_local_imports` and
  `place_other_thread_locals` in `block-wasm-host/src/dylink.rs`, `threadLocalImports` and
  `placeOtherThreadLocals` in `block-app/web/dylink.js`. A spawned thread's blocks for the modules
  above the one that holds libc are allocated on the spawning thread, in `thread-spawn`, before
  the thread runs; the libc module's block is wasi-libc's, read from the start argument.

`crates/tls-diamond` holds the test of it: base, left and right as libraries (`wasm_library` in
`buck/wasm/defs.bzl`, the package's crate linked whole through its `library.rs`; base as the SDK is
linked) and a plugin needing all three (`linked_plugin`), whose generics reach a thread-local in
each, on the main thread and on a spawned one. block-wasm-host's
`the_thread_local_diamond_runs_on_every_thread` runs it on wasmtime and `//crates/tls-diamond:web-test`
in Chromium, each with a small loader for a list of modules of its own. A library's `be3.lib`
section lists the archives it linked, which a link that needs it leaves out.

Still to wire up: editors and the SDK keep the reserve above until the loaders link a list of
modules (plans/plugin_libraries.md, Order 4); then `dylink.rs` and `dylink.js` take over
the test loaders' thread-local part, the loader allocates every thread's blocks from a region it
manages and frees them when the thread ends (the tests' blocks are never freed), and the
reserve, the mirror object and `PLUGIN_TLS` go.

One module stays the root of every linked closure: the one holding std and wasi-libc. Rust's
prebuilt std for wasm32-wasip1-threads and both wasi-libc builds for it (Rust's self-contained
one and `third-party/wasi`'s) are compiled with absolute addresses (`R_WASM_MEMORY_ADDR_SLEB`),
which rust-lld refuses in a `-shared` link; only wasm32-wasip1's wasi-libc has a
position-independent `libc.so`. So the root is linked at fixed addresses, and brings its own
static data, stack, `__stack_pointer`, heap (`__heap_base` to the memory's end, then
`memory.grow`), table and main-thread block. The loader owns everything above it: it creates the
memory from the root's import, places every other module's static data in pages it grows the
memory by before that module's code runs, and gives each module its table slots. Making the
root position-independent too needs std built from source (`-Zbuild-std`) and wasi-libc built
with `-fPIC` for threads.

## Loading

`crates/wasm-dylink` holds the names the link and the loaders agree on, and plans the link:
where the plugin's memory and table slots go, and what answers each of its imports. The plan is
plain data. `crates/block-wasm-host/src/dylink.rs` (wasmtime) and
`crates/block-app/web/dylink.js` (the browser, which reaches the planner through
block-gpu-shim) only carry it out:
1. Instantiate the SDK against the plugin's shared memory, and run its constructors.
2. Plan: grow the memory by the pages the plugin's static data needs (`Declared::pages`) and
   place it there, where the SDK's allocator never reaches (wasi-libc's `sbrk` grows from
   wherever the memory ends, and dlmalloc takes what it gets as a new segment), and pick table
   slots after the SDK's own for the plugin's functions and for every function whose address it
   takes.
3. Instantiate the plugin, answering its `env` imports with the SDK's exports, `GOT.mem` with
   addresses and `GOT.func` with table slots, and apply its data relocations.
4. Set up thread-local storage as above, then run the plugin's constructors.

A thread the plugin spawns instantiates the SDK and then the plugin again from the same plan,
with the thread-local block wasi-libc made for it (read from its start argument). Everything is
linked when the plugin starts, before any thread exists, since a table cannot be shared between
threads: there is no loading of another module later.

Natively the host keeps one compiled SDK per identity (`Host` in block-wasm-host; one wasmtime
engine per cache directory), so every plugin after the first only compiles itself. In a browser
each plugin's worker fetches and compiles the SDK itself.

## The raw plugin interface

A plugin needs no SDK, no Rust and no library: a standalone plugin is one plain wasm module, with
no `dylink.0` section and no `__be3_sdk_*` export, which the host instantiates as it is, on
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

A plugin linked against the SDK follows the linked-module convention on top of this, which
belongs to the link, not to the interface: it and the SDK import one shared `env.memory`, the
plugin exports `plugin_initialize_tls(size, align)` (block-editor-plugin's `platform_entry!`),
which the host calls with the SDK's `__tls_size` and `__tls_align` to set up the main thread's
storage, and the exports and imports under "How the links are made" and "Loading".

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
names one. The host refuses the rest against its own device. A query set, indirect draw
or bind group gains the same check (timestamps, `indirect-first-instance`,
`float32-filterable`) when it is added.

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
textures, `clear_buffer`, and implicit bind group layouts (`layout: None` and
`get_bind_group_layout`) are recorded as wgpu records them. Textures stay on the gpu;
only a mapped read crosses into the plugin's memory.

A *pure guest* has only those imports: no gpu, no SDK, no WASI, no threads, no shared memory, no
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
