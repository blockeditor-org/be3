# Apps linked against any set of libraries

Before this plan every app was linked against exactly one shared module, the
app SDK (`crates/block-plugin-sdk`), and that SDK held beui. An
app that did not use beui, such as `crates/be-app-example-triangle`, still loaded
all of it, an app could not share a library of its own with other apps, and
an app written in another language still had to take Rust's conventions.
This plan has three parts:

1. Dynamic linking of any set of shared libraries, none of them special: an
   app names the libraries it needs (for Rust apps, base and beui), or
   none and is one module written against the raw app interface.
2. A written policy for the app GPU API, and filling the gaps in
   `be-wasm-gpu-guest` that it implies.
3. Moving the framework-neutral parts of the beui editor stack below beui, so an
   app written with another framework would need only its own views and a
   thin adapter.

Not in this plan: an egui (or any other framework's) adapter. Every feature in
the docking and overlay APIs would have to be built twice, and those APIs are
still changing. Revisit once they settle. The work here should leave an adapter
as a single new crate on top of the base library.

Also not in this plan: third-party apps, and publishing libraries for them
to build against. That comes after the app APIs stabilize, the interface is
hardened and permission prompts exist.

Implemented (October 2026); guides/app_libraries.md describes the result.
Base is `//crates/be-sdk-library-base:module` (the crate that was
block-plugin-sdk) and beui `//crates/be-sdk-library-beui:module`. Left for
later, as above: the egui adapter, moving docks and overlays (see "Wait for
docking and overlays"), third-party libraries, and external textures, which
wait on the **(ask)** in part 2.

What changed from the plan:
- **Base is a fixed-address root, not a library like any other.** Rust's
  prebuilt std for wasm32-wasip1-threads and both threaded wasi-libc builds use
  absolute data relocations, which rust-lld refuses in a `-shared` link, so the
  module holding them cannot be position-independent. The user chose to keep
  it as the root: base keeps its own static data, `__stack_pointer`, heap,
  table and main-thread thread-local block; the loader creates the memory and
  places everything above it, and a plan is the list [root, libraries...,
  app] (a standalone app is a list of one). Removing the root needs
  `-Zbuild-std` and a threaded wasi-libc built with `-fPIC`.
- **Thread-locals** take the second route: LLVM forces local-exec on wasm, so
  `be-wasm-link` rewrites each TLS relocation against a symbol nothing in the link
  defines into a read of an imported mutable global `GOT.tls.<symbol>`. Only the
  constant is swapped and the code still adds its own `__tls_base`, so the slot
  holds the symbol's address on that thread minus the importing module's
  `__tls_base`, set per thread after `__wasm_init_tls`.
- **`libraries` lists the whole closure**, not just the direct needs, in any
  order; `be-wasm-link` fails on a needed library the link does not name.
- **C code is compiled `-fPIC` for every guest**, since FreeType and HarfBuzz now
  sit above the root. Base carries libc++, libc++abi and setjmp whole for the
  C++ above it, and a C library above base that calls a libc function base does
  not force in fails the link until it is added to `LIBC` in be-wasm-link.
- **The GPU**: buffer mapping and read-back were a gap too, and were filled with
  the rest; wgpu is overlaid for custom render bundles and the browser's
  occlusion query sets. Render bundle encoders have no debug markers, since
  wgpu 29 has no API for them.
- **Part 3** landed as modules, not crates: the editor state in
  be-sdk's `editor_state`, the region driver as
  `be_sdk::driver`. `related_content.rs` and the memo-based walks
  in `editor/settings.rs` and `editor/tags.rs` stayed in be-sdk-beui, and
  anchored views, floated windows and scrims stay in be-sdk-beui-platform with
  the overlays.
- **Performance**: be-host-bench measured base and beui within noise of the one
  SDK. Its one regression, saves sometimes 60 ms slower, was a race in the
  block store, not the split: a revision listing ran inline in the worker and
  held a seal behind two round trips. Listings now run beside the worker.

## What existed before

- **One SDK per app.** `--be3-sdk` (`crates/be-wasm-link`) links the SDK whole
  with `--export-all` and writes a `be3.sdk` custom section listing the archives
  it linked and its thread-locals. `--be3-plugin=SDK` links an app `-shared`,
  leaves out those archives, checks that the SDK exports every import, and
  exports `__be3_sdk_<sha256>` and `__be3_dylink_<memory>_<table>`. The rule is
  `app_sdk()` and `editor()` in `buck/wasm/defs.bzl`, which hard-codes
  `APP_SDK = "root//crates/block-plugin-sdk:module"`.
- **The loaders.** `crates/be-wasm-dylink` plans the link as plain data: memory from
  the SDK's `malloc`, table slots after the SDK's, `env` imports from the SDK's
  exports, `GOT.mem` and `GOT.func`, data relocations. `be-wasm-host/src/dylink.rs`
  (wasmtime) and `be-host/web/dylink.js` (the browser, through
  be-wasm-gpu-shim) carry it out for exactly two modules. A spawned thread
  instantiates both again from the same plan. Natively one compiled SDK is kept
  per identity.
- **Thread-locals.** LLVM compiles a thread-local access on wasm as a
  local-exec offset from the module's own `__tls_base`. An app reaches the
  SDK's thread-locals through generics it instantiates, so the SDK's block ends
  in a 16 KiB reserve (`APP_TLS`, `APP_TLS_SIZE` in `be-wasm-dylink`), the
  app's link starts with a generated object placing each SDK thread-local at
  the SDK's offset, and the host points the app's `__tls_base` at the SDK's
  block. This works only because there is one module above the SDK.
- **C libraries.** libc++ and libc++abi are linked whole (`WHOLE_LIBRARIES` in
  `be-wasm-link`); the C math library and `LIBC` functions are forced in. An app's
  own C library (third-party/pdfium) stays in the app.
- **The gpu.** `be-wasm-gpu-guest` is wgpu's custom backend over the `be3:gpu`
  world. `objects.rs` has 30 `unimplemented!`s; some say "not available to
  apps" with no reason behind them, others "not yet". The device reports
  `Features::empty()`, and limits that forward a few of the host's (buffer
  sizes, vertex attributes, offset alignments) over
  `Limits::downlevel_defaults()` (`objects.rs` around line 160).
- **The editor stack.**
  - `be-sdk-api` (wire types) and `be-sdk` (`App`,
    `Instance`, `ViewHandler`, `Frame`, `EditorHost`) know nothing of beui, but
    be-sdk is built on `beui-reactive`, the signal graph under beui,
    and re-exports it. Its content API (`ContentProjection` in `content.rs`:
    `project`, `field`, `object`, `objects`, `project_keyed`) hands out
    `ReadSignal`s and `KeyedStore`s, `ready`, `read` and the block queries
    return `beui_reactive::Ready`, and the block graph (`graph.rs`) and one host
    read in `host.rs` call `revision.get()` on an `OwnedSignal` so a reactive
    scope re-runs when they change. About 49 files in the `crates/be-app-*` editors use the
    content projections.
  - `be-sdk-beui-platform` (about 1.4k lines) runs beui's `Runner` behind
    `ViewHandler`: input, per-view surfaces, the `Frame` accounting (windows,
    floats, scrims, claims, descriptions), and host services in `platform.rs`
    (clipboard, paste, file pick and save).
  - `be-sdk-beui` (about 7.5k lines) is the `BeuiRoot` trait, `Editor`
    and its signals over `EditorHost` (`editor.rs`), docks (`docked.rs`, on
    `beui::dock::unstyled`), dialogs, chrome, child blocks, picking, the file
    chooser, layered settings, version history.

## 1. Shared libraries

### Requirements

- **No library is special.** The loader owns what the SDK owns for the process
  today: the memory, the function table, the stack pointer, each thread's
  thread-local blocks, and where each module's static data goes. A library
  brings code and data, and possibly an allocator, and nothing else. A Rust
  app links the Rust libraries because it chose to, not because the host
  needs them.
- **An app may link no library at all.** The raw app interface is enough:
  be-wasm-guest-abi's exports (`app_start`, `app_step`,
  `app_shutdown`) and `be3_host` imports, the app's WIT world
  (`crates/be-sdk-api/wit`, postcard frames as in guides/type_modules.md)
  and the `be3:gpu` imports. A triangle written in Zig, C or WAT loads as one
  module, without Rust, without wasi-libc's conventions, and without threads or
  WASI unless it uses them.
- Today an editor guest has to export `app_initialize_tls`, which
  be-sdk's `platform_entry!` defines for Rust's thread-locals, and
  import a shared `env.memory` (`be-wasm-host/src/lib.rs` around lines 473
  and 602). Both become part of the linked-module convention below, not of the
  app interface. A standalone app imports or exports whichever memory it
  likes, and shared memory is needed only by an app that spawns threads.

### Two kinds of app module

- **Standalone**: a plain wasm module with no `dylink.0` section and no
  libraries. The host instantiates it as it is, answering `be3_host`, the
  app world and `be3:gpu`, plus WASI and `wasi:thread-spawn` if it imports
  them.
- **Linked**: a position-independent shared module (`dylink.0`) whose `be3.lib`
  section names the libraries it needs. Every library is such a module too,
  linked with `--export-all`.

An app is either kind; a library is always linked. Building the same Rust
app both ways (all its crates linked into it, or against libraries) is a
build flag, not a code change.

### Identity and metadata

- A library's identity is the SHA-256 of its module. The module names the
  identities of the libraries it needs, so one hash pins its whole closure.
  Staged as `lib-<sha256>.wasm` (and `.cwasm`), and checked by both hosts as
  SDKs are today.
- `be3.sdk` becomes `be3.lib`, written into every linked module: the libraries
  it needs, the crates and archives it contains, and its thread-local symbols.

### What the loader owns

- **Memory and static data.** The loader creates the memory (shared when any
  module in the closure may spawn threads) and places every module's static
  image itself, from the sizes and alignments in `dylink.0`, before any code
  runs. Today the app's image comes from the SDK's `malloc`; with no special
  module there is no `malloc` to ask yet. The images sit at the bottom; the
  rest of memory is the heap, handed to the closure's one allocator through
  `__heap_base` and `__heap_end` as wasi-libc's dlmalloc expects. Two
  allocators in one closure are refused by the crate-ownership rule below.
- **The table.** The loader creates it and gives each module its slots.
- **The stack.** One `__stack_pointer` global the loader creates, shared by
  every module of a thread, over a stack the loader places for the first
  thread.
- **Thread-local blocks.** One block per module per thread, allocated by the
  loader from its sizes and alignments. Today wasi-libc's `pthread_create`
  allocates a new thread's stack and the SDK's block; with several modules, the
  loader allocates the other modules' blocks when the thread starts and frees
  them when it ends, from a region it manages. The spike below settles how.

### Libraries for Rust apps

Coarse, since each library costs a fetch, a compile per worker in the browser
and an instantiation per thread:

| Library | Holds |
|---|---|
| base | std, alloc, the allocator, the panic runtime, wasi-libc, libc++ and libc++abi (whole), the forced C math and libc functions, wgpu with the custom backend (be-wasm-gpu-guest), be-wasm-ops, be-sdk-api, be-sdk, geometry, serde, postcard, serde_json, uuid |
| beui | reactive, beui and everything under it, be-sdk-beui-platform, be-sdk-beui, FreeType, HarfBuzz |

serde, postcard and uuid go in base: be-sdk-api and be-wasm-ops encode every
frame with postcard over serde, and block ids are uuids. `beui-reactive` does not:
only a retained, fine-grained framework needs signals, and an immediate-mode
one rereads its state every frame and only needs to know when to run one.
A framework with signals of its own (SolidJS-style, say) would want its own,
not `beui-reactive`'s, so `beui-reactive` is beui's and goes with it. Taking it out of
base needs the refactor in "Reactivity out of base" below. Triangle links base;
every beui editor links base and beui. Split base further only when a module
needs part of it, for example a type module that wants std without wgpu.

### Rules the linker enforces

- **Each crate and each C library lives in exactly one module of an app's
  closure.** Two copies of `beui-reactive` would give two reactive runtimes, which
  fails silently. Buck computes a library's crates as its dependency closure
  minus the closures of the libraries it needs; `be-wasm-link` refuses a closure
  in which two modules share a crate or an archive, naming it.
- **Every import resolves to exactly one export in the module's closure**, or
  to the host. A missing symbol fails the link with its name, as today; a
  symbol two libraries define fails too.
- **Constructors run in dependency order.**

### Thread-locals (do this first, as a spike)

The current scheme cannot extend past a chain. In a chain, each library's
thread-locals would follow those of the libraries below it, all inside the
lowest module's reserve, which becomes a hard cap on everything above it. In a
diamond (an app needs A and B, both need base), A and B are linked
separately, both place their thread-locals right after base's, and they
overlap. Base and beui alone are a chain, but the first library a few apps
share (pdfium, for example) makes a diamond.

The target: each module has its own thread-local block and `__tls_base`, and an
access to another module's thread-local goes through a slot the loader fills,
as ELF's dynamic TLS model does. In wasm this fits naturally: every thread
instantiates every module again and globals are per instance, so an imported
global `GOT.tls.<symbol>` can hold that thread's absolute address, set by the
loader per thread the way `GOT.mem` is set now.

The open question is how to make the code use that slot:
- a rustc/LLVM TLS model for wasm that goes through the GOT for symbols another
  module defines, if one exists or can be enabled; or
- `be-wasm-link` rewriting, in the objects before rust-lld, each TLS relocation
  (`R_WASM_MEMORY_ADDR_TLS_SLEB` and friends) against an undefined symbol into
  a `GOT.tls` global read.

Spike: two libraries that both need a third, and an app needing both; a
generic in the app reaching a thread-local in each library; a spawned thread
doing the same, with the loader allocating its blocks; on wasmtime and in the
browser. If neither route works, **(ask)** whether to ship chains only, with a
larger reserve, and refuse diamonds at link time.

### Loading

`be-wasm-dylink`'s plan becomes a list of modules in dependency order, each with
its static image address, table slots, imports (resolved against its closure),
GOT entries, data relocations and thread-local layout; a standalone app is a
list of one with nothing to relocate. `dylink.rs` and `dylink.js` loop over it.
A spawned thread instantiates the whole list again. Natively the host keeps one
compiled module per identity per engine; in the browser, cache compiled modules
across workers if the platform allows it.

### Reactivity out of base

be-sdk keeps its state and drops `beui-reactive`:
- Reads return plain values: the content as `read(&C)`, a loading state as a
  plain enum in place of `beui_reactive::Ready`, the graph's query results and
  known blocks as values.
- Changes are announced, not tracked: each piece of state keeps its revision
  counter (the graph's `revision`, the content's revision) and a list of
  change callbacks, and the existing `on_touched` keyed watchers in
  `content.rs` stay as they are, since they do not use signals. A change also
  wakes the app, as now.

These callbacks are the boundary every framework builds on: an immediate-mode
one ignores them and runs a frame on the wake, and a framework with its own
signals turns them into its signals. So make them easy to adapt:
- as fine-grained as the state: per `Touched` key for content, per query for
  the graph, per watch for host values, so an adapter makes one signal per
  thing a view reads, not one for everything;
- delivered once per change, after the state is consistent (after a whole
  operation or delivery has been applied), so an adapter can batch;
- returning a handle that unsubscribes when dropped, so a signal that is
  cleaned up stops listening.

beui's adapter, in be-sdk-beui, puts `beui-reactive` signals back on top:
`ContentProjection`'s `project`, `field`, `object`, `objects`,
`project_keyed` and `ready` become signals fed by those callbacks, and a graph
or host read tracks a signal its callback bumps. be-sdk-beui exports
them under the names editors use now, so the editors' imports change in one
place and their code does not. A second framework writes its own adapter over
the same callbacks.

Do this before splitting the SDK into base and beui (step 5 in Order), since
the split needs it.

### Build rules

- `wasm_library(libraries = [...])` in `buck/wasm/defs.bzl` replaces
  `app_sdk()`.
- `editor(libraries = [...])` replaces the hard-coded `APP_SDK`.
  `libraries = []` builds a standalone module. Default to base and beui so
  existing editors' BUCK files need no change; triangle names base.
- `buck/program/stage.sh` stages every library in every staged app's closure.
- `./scripts/buck run //:buckify` keeps working: a library's Cargo.toml lists
  its crates as today's `block-plugin-sdk/Cargo.toml` does.

### Performance

Every library is now a position-independent shared module, including the code
that today sits in the SDK's own link, and reaches its globals through the GOT.
Measure base and beui against today's SDK with `be-host-bench`
(guides/performance.md) before landing the split, and record the result in the
PR.

### Tests

- A standalone guest in WAT, in be-wasm-host's tests, that exports no
  `app_initialize_tls`, has its own non-shared memory, and paints through
  `be3:gpu`, on both hosts.
- Triangle built standalone (`libraries = []`) and against base, both loading
  and painting the same.
- The diamond from the spike, on both hosts, kept as a test.
- A link that puts one crate in two modules fails, naming the crate.
- A missing symbol fails the link, naming it and the module that imports it.
- A library whose `.wasm` does not hash to its name is refused by both hosts.
- `//crates/be-host-tests:test` passes with base and beui.

### Guides

Rewrite guides/app_libraries.md around libraries (its title, "What is in it",
"Identity", "How the links are made", "Thread-locals", "Loading"). Its "Guests
without the SDK" section becomes the raw app interface: everything a
standalone app in any language imports and exports, and which of it is
optional. Update the mentions of "the app SDK" in
guides/adding_an_app.md and guides/buck2.md.

## 2. The app GPU API

### Policy

Add to the "The gpu" section of guides/app_libraries.md:

> The app GPU API is all of wgpu that browser WebGPU exposes, core and
> optional features, with the optional features and limits the host's device
> has. Anything missing from that set is a gap, and its `unimplemented!` says
> "not yet". Apps are untrusted, so whatever wgpu marks `unsafe` is refused
> for good: passthrough shaders and pipeline caches. Native-only extensions
> (mesh shaders, ray tracing, multi-draw-indirect with counts) are added only
> behind a feature the app checks, since the same app runs in a browser.

### Gaps to fill

Each of these is core WebGPU:
- compute pipelines and compute passes;
- query sets (occlusion; timestamps behind their feature, coarsened by the
  native host as browsers coarsen them);
- render bundles;
- implicit bind group layouts (`layout: None`, `get_bind_group_layout`);
- buffer-to-buffer, buffer-to-texture, texture-to-buffer and texture-to-texture
  copies;
- `clear_buffer`;
- `draw_indirect` and `draw_indexed_indirect`.

Each one needs a func in the `be3:gpu` WIT (`crates/be-wasm-gpu-abi/wit`), the
guest call in `be-wasm-gpu-guest`, the host side in `be-wasm-gpu-host`, and the
browser side in `be-wasm-gpu-shim`. Add a test app per group that draws or
reads back a result, so each works on both hosts.

### Features and limits

`features()` returns `Features::empty()`. Forward the host device's features,
filtered to those browser WebGPU can offer (timestamp queries, `shader-f16`,
texture compression formats, float32-filterable, ...), and all of its limits
rather than the few forwarded now over `downlevel_defaults()`. An app asks for what it needs when it requests its
device; a feature it did not ask for is refused as wgpu refuses it.

### Messages

- Passthrough shaders and pipeline caches: say they are refused because
  apps are untrusted.
- Mesh shaders, acceleration structures, multi-draw-indirect counts, immediate
  data and `clear_texture`: say they are not in browser WebGPU. Immediate data
  should follow once browsers ship it; `clear_texture` may be emulated in the
  guest with a cleared render pass if an app needs it.
- External textures and external image copies are core WebGPU, but need a
  design for what the source is (a host video frame, a camera, a decoded
  image) that keeps the texture on the GPU. **(ask)** before designing it;
  until then the message says it is not yet available.

## 3. Framework-neutral editor crates

Goal: everything an app needs from the host that is not a view lives below
beui, so a second framework would add views and a thin adapter only.

### Editor state

Take the parts of `be-sdk-beui` that are state over `EditorHost` rather
than views, and split each in two: the logic (the calls to the host, the
bookkeeping) moves into be-sdk as plain state with change
callbacks, and the signals over it stay in be-sdk-beui as part of its
adapter. The candidates:
- `Editor`, `OwnedEditor`, `Artifacts`, `Creation` and `ChildState` from
  `editor.rs`, which has no `view!`. It still uses `beui::native::Document`,
  `EmbedSlot` and `CanvasView`; split those uses off into what stays in
  be-sdk-beui.
- `fitting.rs` and `related_content.rs`, which use only `beui::reactive` and
  geometry.
- The data halves of `version_control.rs` and `layered_settings.rs`.

Every framework then uses be-sdk for the logic and adapts its
change callbacks to its own way of updating.

### The region driver

Split `be-sdk-beui-platform` into a framework-neutral driver and a beui layer.
The driver keeps per-region surface lifetime, the `Frame` accounting and the
host services from `platform.rs`, behind a small trait a framework implements
(roughly: take input for a region, run a frame for it, paint it into a
`PaintTarget`). The beui layer implements that trait over `Runner`.

### Wait for docking and overlays

`docked.rs` is built on `beui::dock::unstyled`, and the floats, scrims and
window requests in the `Frame` accounting are the overlay API. Both are still
changing. Leave them where they are, and move them only once they settle, in
the same change that would let a second framework use them.

## Order

1. Thread-local spike (part 1). It decides whether part 1 is diamonds or
   chains.
2. GPU policy in the guide, the messages, and forwarding features and limits
   (part 2). Independent of everything else; can run in parallel.
3. Standalone apps: the loader owns memory, table, stack and thread-local
   blocks, `app_initialize_tls` and shared memory become optional, and the
   WAT guest and standalone triangle tests pass.
4. `be3.lib`, the closure checks and the N-module plan in `be-wasm-link` and
   `be-wasm-dylink`; both loaders; the build rules. Today's SDK becomes one
   library, to land the machinery without moving code.
5. Reactivity out of base (part 1). Then split the SDK into base and beui.
   Measure. Move triangle onto base alone.
6. Fill the GPU gaps, one group per PR.
7. Editor state and the region driver (part 3), except docks and overlays.
