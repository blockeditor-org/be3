# The plugin SDK

Every plugin is linked against one shared module, the plugin SDK (`crates/block-plugin-sdk`),
instead of carrying its own copy of std, beui, wgpu, FreeType, HarfBuzz and the rest. A plugin
module holds only its own code and the generics instantiated for its own types; the host links
it to the SDK when it loads it, on wasmtime and in the browser alike, from the same files.

## What is in it

`crates/block-plugin-sdk` is a crate that depends on what the plugins have in common: beui,
block-editor-beui, beui-platform-plugin (the runtime block-editor-beui reaches through a symbol
rather than a dependency, which brings beui-host: the renderer, fonts and inspector), block-editor-plugin, serde, serde_json and uuid, and through them
everything underneath. All of it is linked into the SDK whole and exported, since the SDK cannot
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

## Thread-locals

LLVM compiles a thread-local access as an offset from the module's own `__tls_base`
(local-exec) on every target but Emscripten, and that offset cannot point into another module.
A plugin reaches the SDK's thread-locals through generics it instantiates, so the two share one
thread-local block instead:
- The SDK's link ends its thread-locals with a reserve, `__be3_plugin_tls`
  (`PLUGIN_TLS_SIZE`, 16 KiB), in every thread's block.
- A plugin's link starts its thread-locals with a generated object that defines each of the
  SDK's at the SDK's offset, so the plugin's own come after them, inside the reserve.
- The host points the plugin's `__tls_base` at the SDK's block for each thread and copies the
  plugin's own initial values into the reserve. The link fails if a plugin's own thread-locals
  outgrow it.

## Loading

`crates/wasm-dylink` holds the names the link and the loaders agree on, and plans the link:
where the plugin's memory and table slots go, and what answers each of its imports. The plan is
plain data. `crates/block-wasm-host/src/dylink.rs` (wasmtime) and
`crates/block-app/web/dylink.js` (the browser, which reaches the planner through
block-gpu-shim) only carry it out:
1. Instantiate the SDK against the plugin's shared memory, and run its constructors.
2. Plan: allocate the plugin's memory with the SDK's `malloc`, and pick table slots after the
   SDK's own for the plugin's functions and for every function whose address it takes.
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

## Guests without the SDK

Every wasm guest, an editor plugin or a module with no GPU, speaks the same small ABI, in
`crates/block-guest-abi`: it exports `plugin_start`, `plugin_step` and `plugin_shutdown`, and
imports `host_send`, `host_receive`, `host_wake` and `host_panic` from `be3_host`. A guest
implements `block_guest_abi::Guest` (`start` and `step`, with the frames in and out as plain
bytes) and `block_guest_abi::guest!(Type)` exports it; the panic hook, the receive buffer and the
wake flag are the crate's. block-editor-plugin's `plugin!` is one such guest, plus the gpu, WASI
and thread-local storage an editor has.

### The gpu

An editor's wgpu calls are the `be3:gpu` world (`crates/block-gpu-abi/wit`, shaped after
wasi:webgpu), which be-ops-bindgen generates with `local` (guides/type_modules.md,
"Operations and WIT"): its imports, module `be3:gpu/device@0.1.0`, are answered on the
plugin's own thread, so they return their results, and a call made per draw takes only
wasm values. block-gpu-guest is wgpu's custom backend over the guest bindings (a
surface is the host's surface id; `release_surface` drops it), block-gpu-host's `Gpu`
implements `Host` over a real wgpu device, block-wasm-host's `State` is the `Caller`
and links the glue, and block-gpu-shim is the `Caller` in a browser worker and exports
each import as `module#name`, which `plugin.js` binds every import it does not answer
itself to. A call whose arguments cannot be read is refused into the device's error,
which `take-error` and the step both report; natively a plugin that reaches for a gpu
no device could be opened for stops with why. `example-gpu-calls` in block-wasm-host
times the per-draw calls (guides/performance.md).

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
  reaches the host. Dropping it interrupts the guest. Editors (`plugin_host/wasm.rs`) and pure guests both use it.
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
