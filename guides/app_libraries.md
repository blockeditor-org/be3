# App libraries

An app is either **standalone**, one plain wasm module with everything in it
(written against the raw app interface below, in any language), or
**linked**: a position-independent module naming the libraries it needs,
which the host links it to at load time, natively on V8 and in the browser,
from the same files. A library holds what apps share (std, beui, wgpu, beui's
fonts, ...), so a linked app's module holds only its own code and the
generics instantiated for its types, and every app shares one file and one
compiled module per library.

## The libraries

A Rust app links two libraries, or one (`buck/wasm/defs.bzl`):

- **base**, `crates/be-sdk-library-base`, the root: std, wasi-libc, libc++,
  setjmp, wgpu with its custom backend, be-wasm-ops, be-sdk-api, be-sdk,
  serde, postcard, uuid and what they need. Nothing in it depends on beui;
  be-sdk's state announces changes through callbacks any UI framework adapts
  (guides/blocks/apps.md, State and change callbacks).
- **beui**, `crates/be-sdk-library-beui`, above base: beui-reactive, beui,
  be-sdk-beui and be-sdk-beui-platform (renderer, fonts), with the bundled
  faces and the Rust crates that shape and draw them (skrifa, harfrust, zeno),
  and Vello's GPU renderer (`vello_gpu` over wgpu, and `vello_common`), which
  an app may also draw with directly. Its features are set in the library's
  `Cargo.toml`: no `text`, `webgl`, `png`, `pico_svg` or `multithreading`, so
  an app that names vello_gpu or vello_common must not turn those on.

Every editor links both (`DEFAULT_LIBRARIES`) unless its `app()` says
otherwise, and its tests link a third (Tests, below); triangle names base
alone (`BASE_LIBRARY`), and
`app(libraries = [])` links an editor standalone. A library is linked whole
and exports everything, since it cannot know what a future app will use, so a
crate only one or two apps use stays in those apps. To move a crate into a
library, add it to the library crate's `Cargo.toml` and `lib.rs` and run
`./scripts/buckify`.

C code: libc++ and setjmp live in base even though base does not use them, so
every C++ library above shares one copy. wasi-libc functions some C library
calls are forced into base (`MATH` and `LIBC` in
`crates/be-wasm-link/src/lib.rs`); a module that calls a C function no library
has fails to link naming it, and adding it there fixes that. Any C a linked
module links must be compiled position-independent, which the apps' toolchain
does (`_wasi_flags` in `buck/toolchains/BUCK` adds `-fPIC`, which cc-rs
leaves out on wasm), so C libraries are built from source on it (as
`third-party/pdfium` builds PDFium); an archive built elsewhere will not link.

## Tests

An app's tests (`app_tests` in `buck/wasm/defs.bzl`: every `app()`'s, and
be-sdk's, be-sdk-beui's, beui-demo's and paint-review's) are one module linked
against the same libraries as the app, so they run against the libraries apps
ship with. Where those hold base and beui they also link **test**,
`crates/be-sdk-library-test`, above beui: be-sdk-test and what only it uses
(be-sdk-type-host, beui-host, the block store) and libtest, none of which an
app ships. A test module then holds only its crate, its tests and the
generics they instantiate: checklist's is 3.3 MB, against 47 MB with
everything in it. `app_tests(libraries = [])` links a standalone test module.

- **libtest.** The toolchain's prebuilt libtest, like std, is not
  position-independent, so it cannot be linked into a `-shared` module.
  `third-party/libtest` compiles libtest and getopts from the toolchain's
  rust-src; every test module names it as its `test` crate, which `rustc
  --test` then links in place of the toolchain's, and the test library links
  both archives whole, so test modules leave them out. Upgrading Rust means
  updating its pins.
- **Link.** rustc `--test` makes a WASI command; the linker drops
  `crt1-command.o` from a linked one, and rustc already exports
  `__main_void`, the test harness's `main`.
- **Run.** The harness (`wasm_test_harness`) passes app-test-runner each
  library in the closure, in order, as `--library`, and the module last.
  `Host::run_tests` links them as it links an app (`src/dylink.rs`), then
  calls `__main_void`, whose result is the exit code; a panic still ends the
  instance, and the rerun one at a time is unchanged.
- **Compile.** V8 compiles each library once in the runner's process and the
  test module as it loads; nothing is compiled ahead of the run.

A standalone module that links two C libraries with the same symbols gets
one definition of each name from whichever archive comes first; linked tests
run on the libraries the app ships instead.

## Identity

Rust has no stable ABI, so a module works only with the exact libraries it was
linked against. A library's identity is the SHA-256 of its module. A linked
module names its whole closure, in dependency order, as exported globals
`__be3_lib_<position>_<identity>`. Libraries are staged as
`lib-<identity>.wasm`; both hosts hash it before use and refuse one whose
hash is not its name.

Every linked module carries a `be3.lib` custom section listing its name, the
libraries it needs, and the Rust archives and C libraries it holds; the
linker reads it.

## How the links are made

Every wasm link goes through `crates/be-wasm-link` (the toolchain's linker),
which otherwise passes it to rust-lld unchanged. The root is linked with
everything exported and at fixed addresses; a library above it
position-independent (`-shared`) with everything exported; a linked app
position-independent with only its own code. `libraries` on a rule names the
whole closure, not only direct uses.

A module above the root is linked `-Bsymbolic`, so what it defines binds to
its own definition: otherwise it reaches each of its symbols another module
could define through a `GOT.func` or `GOT.mem` import, which the loader answers
from the lowest module exporting the name, and a module carrying its own copy
of a C library another module also carries would be handed the other's.

A link that needs libraries reads their `be3.lib` sections and refuses a
closure that is incomplete or in which two libraries hold the same crate or C
library (two copies of beui-reactive would be two reactive runtimes, which
fails silently), leaves the closure's archives out so their symbols become
imports, checks every import resolves to exactly one library, and shortens
mangled symbol names to a hash (`be_wasm_dylink::short_symbol`; the `name`
section keeps readable names for backtraces).

### The root

The root is the module holding std and wasi-libc. Rust's prebuilt std and
wasi-libc are compiled with absolute addresses, which rust-lld refuses in a
`-shared` link, so the root is linked at fixed addresses and owns its data,
stack, heap and table; it imports the memory the loader makes, and the loader
places everything else above it
(decisions/app_library_base_is_a_fixed_address_root.md). Every guest is
single-threaded `wasm32-wasip1` (decisions/guests_are_single_threaded.md), so
that memory is not shared, and LLVM lowers a thread-local to plain data: no
module has a thread-local block or imports another's thread-locals.

## Loading

`crates/be-wasm-dylink` plans the link as plain data (`plan`): where each
module's data goes, its table slots and what answers each import. `crates/be-wasm-host/src/dylink.rs` (natively, on V8 through the JavaScript
WebAssembly API called from Rust) and `crates/be-host/web/dylink.js` (the
browser, through be-wasm-gpu-shim) only carry it out the same way: instantiate the root against the memory and run
`_initialize`, grow the memory for the other modules' data, then instantiate
and relocate each module in order and run its constructors before anything
above it is instantiated, once, on the guest's one thread. Everything is
linked at start and nothing loads later; with no threads to share a table
with, a library could, but nothing asks for it yet.

Natively each library is compiled once per process and shared between the
isolates of every app as a `v8::CompiledWasmModule` (`Host` in be-wasm-host),
so later apps compile only themselves; nothing is cached on disk. In a browser each worker
compiles with `WebAssembly.compileStreaming`, which lets the browser cache it;
the server must serve `.wasm` as `application/wasm` for that.

## The raw app interface

An app needs no Rust and no library: a standalone app is one plain wasm module
with no `dylink.0` section and no `__be3_lib_*` export, which the host
instantiates as it is. `crates/be-wasm-host/src/tests/standalone.wat` is one
that paints through the gpu.

Every guest speaks the ABI in `crates/be-wasm-guest-abi`. It exports
`app_start`, `app_step` and `app_shutdown` (no parameters or results),
optionally `memory` and `_initialize`. It may import,
each only if used:

- `be3_host`: `host_send` and `host_receive` (the frames of its world,
  postcard, guides/type_modules.md), `host_wake` and `host_panic`;
- the `be3:gpu/device@0.1.0` imports (`gpu_local_imports!` in
  be-wasm-gpu-abi lists them);
- one memory, not shared (a guest is single-threaded,
  decisions/guests_are_single_threaded.md), and `wasi_snapshot_preview1`.

Any other import is answered by a function that fails when called. A Rust
guest implements `be_wasm_guest_abi::Guest` and exports it with
`be_wasm_guest_abi::guest!`; be-sdk's `app!` is one.

### Pure guests

A pure guest imports only `be3_host` and the few `wasi_snapshot_preview1` calls
Rust's std makes: no gpu, library, threads, files or clock. Its WASI is
`crates/be-wasm-pure-wasi`, which every host of one answers through (be-wasm-host,
tabletop-host's wasmi and be-wasm-root-host on the page; be-wasm-guest-host's
`guest.js` answers the same in JavaScript): `random_get` is a ChaCha20 stream
from the seed its host gives it (`Host::with_seed`, zero unless given), so a guest
stays deterministic; `fd_write` to standard output or error reaches the host's log a
line at a time, after the guest's name;
its arguments and environment are empty, the clocks answer ENOSYS, and `proc_exit`
stops it. Any other import is refused when it loads. Every host's answers are held to
RFC 7539's vectors (`be_wasm_pure_wasi::vectors`) by be-wasm-host's tests and, through the
same guest (`crates/be-wasm-host/src/tests/pure_wasi.wat`), be-wasm-root-web-testing's.
It is a `pure_guest_crate`'s `:module` (buck/wasm/defs.bzl), a plain
`wasm32-wasip1` cdylib built in the guests' plan with the apps, not an `app()`. A type module
is one, and so is the input remapper, which runs in the sandbox for modules
from bytes (guides/apps/runtime.md, Backends); `crates/be-wasm-guest-echo` is
the smallest. The host refuses any other import and caps its memory. The echo
guest is also built as a staged app linked against the base library (`:app`),
which the sandbox's tests spawn by its id, with crates/be-sdk-worker-echo as
its worker. A worker module (`be_sdk_worker::worker!`) is an ordinary guest
linked as an app is, exporting `app_start` and `app_step` like any other, and so is a root
guest, which links base and beui to draw with beui
(decisions/a_root_links_beui_as_an_app_does.md).

#### What a type module builds

A type module (`:module`) builds only what its replica runs, with the type crates. It is built
in the guests' plan, so a crate it shares with the apps is compiled once for both, with the
apps' features: be-block-store with `vault`, be-block-commit and be-block-session with `store`.
The linker drops what the replica never calls, so those cost no size and import nothing more:
moving every pure guest from `wasm32-unknown-unknown` into the plan took the dev type modules
about 1% smaller (November 2026: chat 4,283,048 to 4,220,554 bytes, checklist 3,802,354 to
3,756,188, text 3,912,275 to 3,868,138), and the smallest guests grew by what wasip1's std
formats and reads (the echo guest 36,186 to 88,468). A pure guest may not link wgpu: its `web`
feature, which be-wasm-gpu-guest turns on and the guests' plan unifies, links wasm-bindgen.
So the ABI's wgpu conversions are `crates/be-wasm-gpu-convert`, apart from
`crates/be-wasm-gpu-abi`. What a type module uses, and why:

| crate | type modules use it for |
|---|---|
| be-block-content | the traits, metadata, `Root`, blobs |
| be-block-model, be-lib-sequence | `Document<R>`, text and lists |
| be-block-commit, `similar` | `MergeResult`; `similar` is the line diff under be-block-model's sequence merge (`merge_lines`; decisions/type_modules_keep_similar.md) |
| be-block-store, sha2, ciborium | `Part`/`PartRef`/`Hash`/`Manifest`, CBOR of the model |
| be-block-session | the order (`Sequencer`) and `SessionMessage` the replica runs |
| be-block-protocol, be-block-graph | `SessionState`, `ClientId`, `RelayKind`; graph only for `Access`/`BlockParent` in the protocol, and `WORKSPACE_TYPE` |
| be-sdk-type-api, be-wasm-ops, be-wasm-guest-abi | the protocol and the guest ABI |

be-block, be-lib-search, be-lib-graphemes, the icu crates and unicode-normalization are in no
type module's build. The apps (`wasi_guest`) build be-block through be-sdk, whose frame reads
its built-ins for every editor (decisions/no_crate_splits_inside_dependency_chains.md).
Shrinking what a module ships is decisions/shipped_bundle_size_settings.md.

### The host half

`crates/be-wasm-guest-host` runs guests for every host: `Worker` queues
frames, steps the guest only when frames wait or it woke, and finds a step
past `STEP_TIMEOUT`. It is settled (which be-host's busy, and so `drive settle`,
reads) while no frames wait and no step works on frames the host sent; a step
the guest woke itself for straight after one that took frames counts as working
on them, since an app takes an answer in one step and acts on it in the next.
A guest that wakes itself later, on its own, does not keep it busy, but one that
wakes itself every step never settles, and `drive settle` times out on it with
"has input or a step in flight".
`thread::spawn` runs one on its own thread natively;
`InProcess` runs a guest's Rust in-process for tests; `web::WebGuest` runs one
in a blob worker (`src/guest.js`), which be-host's `app.js` extends for an
editor with libraries, WASI, threads and the gpu.

## The gpu

An editor's wgpu calls are the `be3:gpu` world (`crates/be-wasm-gpu-abi/wit`,
shaped after wasi:webgpu), generated as a `local` interface
(guides/type_modules.md, Local interfaces): its imports are answered on the
app's own thread and a per-draw call takes only wasm values.

- `be-wasm-gpu-guest` is wgpu's custom backend over the guest bindings.
- `be-wasm-gpu-host`'s `Gpu` implements it over a real wgpu device.
- be-wasm-host links the glue natively; `be-wasm-gpu-shim` is the same in a
  browser worker.

The app GPU API is what browser WebGPU exposes, with the host device's
optional features and limits, since the same app runs in a browser. Anything
missing is an `unimplemented!` in `be-wasm-gpu-guest/src/objects.rs`;
whatever wgpu marks `unsafe` is refused for good, since apps are untrusted.
A native-only feature would be added only behind a feature the app checks.

Traps when changing it:

- An app's device has only the features it requested; the guest
  (`features.rs`) and host both validate against that, not the host device.
- Buffer mapping, `on_submitted_work_done` and compilation info are
  asynchronous on both hosts: `map-async` and `on-submitted-work-done` answer
  tickets, the host wakes the app when one settles or the device is lost, and
  the guest's one `poll` takes everything that settled and answers callbacks
  (`be_wasm_gpu_guest::poll`, which be-sdk calls each step).
- Device loss is per app: `Device::destroy` drops only the app's view of the
  host's shared device until it requests a device again. Natively be-host
  restarts every app on a replaced device (`replace_gpu`).
- Timestamps are converted to nanoseconds and coarsened to 100 µs on the host
  (`queries.rs`), as Chrome does.
- Render bundles are recorded by the host and replayed at `finish`
  (`bundles.rs`).
- `third-party/rust/overlays/wgpu` patches wgpu so a bundle that fails
  validation reaches the device's error sink, and so occlusion queries work in
  the browser; refresh or drop it when wgpu is upgraded.

`example-gpu-calls` in be-wasm-host times the per-draw calls
(guides/performance.md).
