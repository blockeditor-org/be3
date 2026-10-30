# Plugins linked against any set of libraries

Today every plugin is linked against exactly one shared module, the plugin SDK
(`crates/block-plugin-sdk`, guides/plugin_sdk.md), and that SDK holds beui. A
plugin that does not use beui, such as `crates/editors/triangle`, still loads
all of it, a plugin cannot share a library of its own with other plugins, and
a plugin written in another language still has to take Rust's conventions.
This plan has three parts:

1. Dynamic linking of any set of shared libraries, none of them special: a
   plugin names the libraries it needs (for Rust plugins, base and beui), or
   none and is one module written against the raw plugin interface.
2. A written policy for the plugin GPU API, and filling the gaps in
   `block-gpu-guest` that it implies.
3. Moving the framework-neutral parts of the beui editor stack below beui, so a
   plugin written with another framework would need only its own views and a
   thin adapter.

Not in this plan: an egui (or any other framework's) adapter. Every feature in
the docking and overlay APIs would have to be built twice, and those APIs are
still changing. Revisit once they settle. The work here should leave an adapter
as a single new crate on top of the base library.

Also not in this plan: third-party plugins, and publishing libraries for them
to build against. That comes after the plugin APIs stabilize, the interface is
hardened and permission prompts exist.

Not implemented yet. Items marked **(ask)** need a decision from the user during
the work.

## What exists

- **One SDK per plugin.** `--be3-sdk` (`crates/wasm-link`) links the SDK whole
  with `--export-all` and writes a `be3.sdk` custom section listing the archives
  it linked and its thread-locals. `--be3-plugin=SDK` links a plugin `-shared`,
  leaves out those archives, checks that the SDK exports every import, and
  exports `__be3_sdk_<sha256>` and `__be3_dylink_<memory>_<table>`. The rule is
  `plugin_sdk()` and `editor()` in `buck/wasm/defs.bzl`, which hard-codes
  `PLUGIN_SDK = "root//crates/block-plugin-sdk:module"`.
- **The loaders.** `crates/wasm-dylink` plans the link as plain data: memory from
  the SDK's `malloc`, table slots after the SDK's, `env` imports from the SDK's
  exports, `GOT.mem` and `GOT.func`, data relocations. `block-wasm-host/src/dylink.rs`
  (wasmtime) and `block-app/web/dylink.js` (the browser, through
  block-gpu-shim) carry it out for exactly two modules. A spawned thread
  instantiates both again from the same plan. Natively one compiled SDK is kept
  per identity.
- **Thread-locals.** LLVM compiles a thread-local access on wasm as a
  local-exec offset from the module's own `__tls_base`. A plugin reaches the
  SDK's thread-locals through generics it instantiates, so the SDK's block ends
  in a 16 KiB reserve (`PLUGIN_TLS`, `PLUGIN_TLS_SIZE` in `wasm-dylink`), the
  plugin's link starts with a generated object placing each SDK thread-local at
  the SDK's offset, and the host points the plugin's `__tls_base` at the SDK's
  block. This works only because there is one module above the SDK.
- **C libraries.** libc++ and libc++abi are linked whole (`WHOLE_LIBRARIES` in
  `wasm-link`); the C math library and `LIBC` functions are forced in. A plugin's
  own C library (third-party/pdfium) stays in the plugin.
- **The gpu.** `block-gpu-guest` is wgpu's custom backend over the `be3:gpu`
  world. `objects.rs` has 30 `unimplemented!`s; some say "not available to
  plugins" with no reason behind them, others "not yet". The device reports
  `Features::empty()`, and limits that forward a few of the host's (buffer
  sizes, vertex attributes, offset alignments) over
  `Limits::downlevel_defaults()` (`objects.rs` around line 160).
- **The editor stack.**
  - `block-plugin-api` (wire types) and `block-editor-plugin` (`Plugin`,
    `Instance`, `ViewHandler`, `Frame`, `EditorHost`) know nothing of beui, but
    block-editor-plugin is built on `reactive`, the signal graph under beui,
    and re-exports it. Its content API (`ContentProjection` in `content.rs`:
    `project`, `field`, `object`, `objects`, `project_keyed`) hands out
    `ReadSignal`s and `KeyedStore`s, `ready`, `read` and the block queries
    return `reactive::Ready`, and the block graph (`graph.rs`) and one host
    read in `host.rs` call `revision.get()` on an `OwnedSignal` so a reactive
    scope re-runs when they change. About 49 files in `crates/editors` use the
    content projections.
  - `beui-platform-plugin` (about 1.4k lines) runs beui's `Runner` behind
    `ViewHandler`: input, per-view surfaces, the `Frame` accounting (windows,
    floats, scrims, claims, descriptions), and host services in `platform.rs`
    (clipboard, paste, file pick and save).
  - `block-editor-beui` (about 7.5k lines) is the `BeuiApp` trait, `Editor`
    and its signals over `EditorHost` (`editor.rs`), docks (`docked.rs`, on
    `beui::dock::unstyled`), dialogs, chrome, child blocks, picking, the file
    chooser, layered settings, version history.

## 1. Shared libraries

### Requirements

- **No library is special.** The loader owns what the SDK owns for the process
  today: the memory, the function table, the stack pointer, each thread's
  thread-local blocks, and where each module's static data goes. A library
  brings code and data, and possibly an allocator, and nothing else. A Rust
  plugin links the Rust libraries because it chose to, not because the host
  needs them.
- **A plugin may link no library at all.** The raw plugin interface is enough:
  block-guest-abi's exports (`plugin_start`, `plugin_step`,
  `plugin_shutdown`) and `be3_host` imports, the plugin's WIT world
  (`crates/block-plugin-api/wit`, postcard frames as in guides/type_modules.md)
  and the `be3:gpu` imports. A triangle written in Zig, C or WAT loads as one
  module, without Rust, without wasi-libc's conventions, and without threads or
  WASI unless it uses them.
- Today an editor guest has to export `plugin_initialize_tls`, which
  block-editor-plugin's `platform_entry!` defines for Rust's thread-locals, and
  import a shared `env.memory` (`block-wasm-host/src/lib.rs` around lines 473
  and 602). Both become part of the linked-module convention below, not of the
  plugin interface. A standalone plugin imports or exports whichever memory it
  likes, and shared memory is needed only by a plugin that spawns threads.

### Two kinds of plugin module

- **Standalone**: a plain wasm module with no `dylink.0` section and no
  libraries. The host instantiates it as it is, answering `be3_host`, the
  plugin world and `be3:gpu`, plus WASI and `wasi:thread-spawn` if it imports
  them.
- **Linked**: a position-independent shared module (`dylink.0`) whose `be3.lib`
  section names the libraries it needs. Every library is such a module too,
  linked with `--export-all`.

A plugin is either kind; a library is always linked. Building the same Rust
plugin both ways (all its crates linked into it, or against libraries) is a
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
  runs. Today the plugin's image comes from the SDK's `malloc`; with no special
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

### Libraries for Rust plugins

Coarse, since each library costs a fetch, a compile per worker in the browser
and an instantiation per thread:

| Library | Holds |
|---|---|
| base | std, alloc, the allocator, the panic runtime, wasi-libc, libc++ and libc++abi (whole), the forced C math and libc functions, wgpu with the custom backend (block-gpu-guest), be-ops, block-plugin-api, block-editor-plugin, geometry, serde, postcard, serde_json, uuid |
| beui | reactive, beui and everything under it, beui-platform-plugin, block-editor-beui, FreeType, HarfBuzz |

serde, postcard and uuid go in base: block-plugin-api and be-ops encode every
frame with postcard over serde, and block ids are uuids. `reactive` does not:
only a retained, fine-grained framework needs signals, and an immediate-mode
one rereads its state every frame and only needs to know when to run one.
A framework with signals of its own (SolidJS-style, say) would want its own,
not `reactive`'s, so `reactive` is beui's and goes with it. Taking it out of
base needs the refactor in "Reactivity out of base" below. Triangle links base;
every beui editor links base and beui. Split base further only when a module
needs part of it, for example a type module that wants std without wgpu.

### Rules the linker enforces

- **Each crate and each C library lives in exactly one module of a plugin's
  closure.** Two copies of `reactive` would give two reactive runtimes, which
  fails silently. Buck computes a library's crates as its dependency closure
  minus the closures of the libraries it needs; `wasm-link` refuses a closure
  in which two modules share a crate or an archive, naming it.
- **Every import resolves to exactly one export in the module's closure**, or
  to the host. A missing symbol fails the link with its name, as today; a
  symbol two libraries define fails too.
- **Constructors run in dependency order.**

### Thread-locals (do this first, as a spike)

The current scheme cannot extend past a chain. In a chain, each library's
thread-locals would follow those of the libraries below it, all inside the
lowest module's reserve, which becomes a hard cap on everything above it. In a
diamond (a plugin needs A and B, both need base), A and B are linked
separately, both place their thread-locals right after base's, and they
overlap. Base and beui alone are a chain, but the first library a few plugins
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
- `wasm-link` rewriting, in the objects before rust-lld, each TLS relocation
  (`R_WASM_MEMORY_ADDR_TLS_SLEB` and friends) against an undefined symbol into
  a `GOT.tls` global read.

Spike: two libraries that both need a third, and a plugin needing both; a
generic in the plugin reaching a thread-local in each library; a spawned thread
doing the same, with the loader allocating its blocks; on wasmtime and in the
browser. If neither route works, **(ask)** whether to ship chains only, with a
larger reserve, and refuse diamonds at link time.

### Loading

`wasm-dylink`'s plan becomes a list of modules in dependency order, each with
its static image address, table slots, imports (resolved against its closure),
GOT entries, data relocations and thread-local layout; a standalone plugin is a
list of one with nothing to relocate. `dylink.rs` and `dylink.js` loop over it.
A spawned thread instantiates the whole list again. Natively the host keeps one
compiled module per identity per engine; in the browser, cache compiled modules
across workers if the platform allows it.

### Reactivity out of base

block-editor-plugin keeps its state and drops `reactive`:
- Reads return plain values: the content as `read(&C)`, a loading state as a
  plain enum in place of `reactive::Ready`, the graph's query results and
  known blocks as values.
- Changes are announced, not tracked: each piece of state keeps its revision
  counter (the graph's `revision`, the content's revision) and a list of
  change callbacks, and the existing `on_touched` keyed watchers in
  `content.rs` stay as they are, since they do not use signals. A change also
  wakes the plugin, as now.

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

beui's adapter, in block-editor-beui, puts `reactive` signals back on top:
`ContentProjection`'s `project`, `field`, `object`, `objects`,
`project_keyed` and `ready` become signals fed by those callbacks, and a graph
or host read tracks a signal its callback bumps. block-editor-beui exports
them under the names editors use now, so the editors' imports change in one
place and their code does not. A second framework writes its own adapter over
the same callbacks.

Do this before splitting the SDK into base and beui (step 5 in Order), since
the split needs it.

### Build rules

- `wasm_library(libraries = [...])` in `buck/wasm/defs.bzl` replaces
  `plugin_sdk()`.
- `editor(libraries = [...])` replaces the hard-coded `PLUGIN_SDK`.
  `libraries = []` builds a standalone module. Default to base and beui so
  existing editors' BUCK files need no change; triangle names base.
- `buck/app/stage.sh` stages every library in every staged plugin's closure.
- `./scripts/buck run //:buckify` keeps working: a library's Cargo.toml lists
  its crates as today's `block-plugin-sdk/Cargo.toml` does.

### Performance

Every library is now a position-independent shared module, including the code
that today sits in the SDK's own link, and reaches its globals through the GOT.
Measure base and beui against today's SDK with `app-bench`
(guides/performance.md) before landing the split, and record the result in the
PR.

### Tests

- A standalone guest in WAT, in block-wasm-host's tests, that exports no
  `plugin_initialize_tls`, has its own non-shared memory, and paints through
  `be3:gpu`, on both hosts.
- Triangle built standalone (`libraries = []`) and against base, both loading
  and painting the same.
- The diamond from the spike, on both hosts, kept as a test.
- A link that puts one crate in two modules fails, naming the crate.
- A missing symbol fails the link, naming it and the module that imports it.
- A library whose `.wasm` does not hash to its name is refused by both hosts.
- `//crates/app-tests:test` passes with base and beui.

### Guides

Rewrite guides/plugin_sdk.md around libraries (its title, "What is in it",
"Identity", "How the links are made", "Thread-locals", "Loading"). Its "Guests
without the SDK" section becomes the raw plugin interface: everything a
standalone plugin in any language imports and exports, and which of it is
optional. Update the mentions of "the plugin SDK" in
guides/adding_a_plugin_editor.md and guides/buck2.md.

## 2. The plugin GPU API

### Policy

Add to the "The gpu" section of guides/plugin_sdk.md:

> The plugin GPU API is all of wgpu that browser WebGPU exposes, core and
> optional features, with the optional features and limits the host's device
> has. Anything missing from that set is a gap, and its `unimplemented!` says
> "not yet". Plugins are untrusted, so whatever wgpu marks `unsafe` is refused
> for good: passthrough shaders and pipeline caches. Native-only extensions
> (mesh shaders, ray tracing, multi-draw-indirect with counts) are added only
> behind a feature the plugin checks, since the same plugin runs in a browser.

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

Each one needs a func in the `be3:gpu` WIT (`crates/block-gpu-abi/wit`), the
guest call in `block-gpu-guest`, the host side in `block-gpu-host`, and the
browser side in `block-gpu-shim`. Add a test plugin per group that draws or
reads back a result, so each works on both hosts.

### Features and limits

`features()` returns `Features::empty()`. Forward the host device's features,
filtered to those browser WebGPU can offer (timestamp queries, `shader-f16`,
texture compression formats, float32-filterable, ...), and all of its limits
rather than the few forwarded now over `downlevel_defaults()`. A plugin asks for what it needs when it requests its
device; a feature it did not ask for is refused as wgpu refuses it.

### Messages

- Passthrough shaders and pipeline caches: say they are refused because
  plugins are untrusted.
- Mesh shaders, acceleration structures, multi-draw-indirect counts, immediate
  data and `clear_texture`: say they are not in browser WebGPU. Immediate data
  should follow once browsers ship it; `clear_texture` may be emulated in the
  guest with a cleared render pass if a plugin needs it.
- External textures and external image copies are core WebGPU, but need a
  design for what the source is (a host video frame, a camera, a decoded
  image) that keeps the texture on the GPU. **(ask)** before designing it;
  until then the message says it is not yet available.

## 3. Framework-neutral editor crates

Goal: everything a plugin needs from the host that is not a view lives below
beui, so a second framework would add views and a thin adapter only.

### Editor state

Take the parts of `block-editor-beui` that are state over `EditorHost` rather
than views, and split each in two: the logic (the calls to the host, the
bookkeeping) moves into block-editor-plugin as plain state with change
callbacks, and the signals over it stay in block-editor-beui as part of its
adapter. The candidates:
- `Editor`, `OwnedEditor`, `Artifacts`, `Creation` and `ChildState` from
  `editor.rs`, which has no `view!`. It still uses `beui::native::Document`,
  `EmbedSlot` and `CanvasView`; split those uses off into what stays in
  block-editor-beui.
- `fitting.rs` and `related_content.rs`, which use only `beui::reactive` and
  geometry.
- The data halves of `version_control.rs` and `layered_settings.rs`.

Every framework then uses block-editor-plugin for the logic and adapts its
change callbacks to its own way of updating.

### The region driver

Split `beui-platform-plugin` into a framework-neutral driver and a beui layer.
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
3. Standalone plugins: the loader owns memory, table, stack and thread-local
   blocks, `plugin_initialize_tls` and shared memory become optional, and the
   WAT guest and standalone triangle tests pass.
4. `be3.lib`, the closure checks and the N-module plan in `wasm-link` and
   `wasm-dylink`; both loaders; the build rules. Today's SDK becomes one
   library, to land the machinery without moving code.
5. Reactivity out of base (part 1). Then split the SDK into base and beui.
   Measure. Move triangle onto base alone.
6. Fill the GPU gaps, one group per PR.
7. Editor state and the region driver (part 3), except docks and overlays.
