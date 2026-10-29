# Plugins linked against any set of libraries

Today every plugin is linked against exactly one shared module, the plugin SDK
(`crates/block-plugin-sdk`, guides/plugin_sdk.md), and that SDK holds beui. A
plugin that does not use beui, such as `crates/editors/triangle`, or a
third-party plugin written with another UI framework, still loads all of it, and
cannot bring a library of its own that the SDK would share. This plan has three
parts:

1. Dynamic linking of any set of shared libraries, so the SDK becomes a root
   runtime plus libraries (gpu, block-editor-plugin, beui, ...) and a plugin
   names the ones it needs.
2. A written policy for the plugin GPU API, and filling the gaps in
   `block-gpu-guest` that it implies.
3. Moving the framework-neutral parts of the beui editor stack below beui, so a
   plugin written with another framework would need only its own views and a
   thin adapter.

Not in this plan: an egui (or any other framework's) adapter. Every feature in
the docking and overlay APIs would have to be built twice, and those APIs are
still changing. Revisit once they settle. The work here should leave an adapter
as a single new crate on top of the block-editor-plugin library.

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
    `Instance`, `Regions`, `Frame`, `EditorHost`) know nothing of beui, apart
    from re-exporting `reactive`.
  - `beui-platform-plugin` (about 1.4k lines) runs beui's `Runner` behind
    `Regions`: input, per-region surfaces, the `Frame` accounting (windows,
    floats, scrims, claims, descriptions), and host services in `platform.rs`
    (clipboard, paste, file pick and save).
  - `block-editor-beui` (about 7.5k lines) is the `BeuiApp` trait, `Editor`
    and its signals over `EditorHost` (`editor.rs`), docks (`docked.rs`, on
    `beui::dock::unstyled`), dialogs, chrome, child blocks, picking, the file
    chooser, layered settings, version history.

## 1. Shared libraries

### The model

- **A root module, the runtime**: std, alloc, the global allocator, the panic
  runtime, wasi-libc, libc++ and libc++abi, the forced C math and libc
  functions. Each of these must exist once per process. The root owns memory,
  `malloc`, the stack and the function table, as the SDK does today.
- **Libraries**: position-independent shared modules, linked with
  `--export-all`, each naming the libraries it needs. A plugin is a library that
  also exports the guest ABI (`plugin_start`, `plugin_step`, ...).
- **Identity**: a library's identity is the SHA-256 of its module, and the
  module names its needed libraries' identities, so one hash pins its whole
  closure. Staged as `lib-<sha256>.wasm` (and `.cwasm`), checked as SDKs are
  today.
- **Metadata**: `be3.sdk` becomes `be3.lib`: the libraries needed, the crates
  and archives the library contains, and its thread-local symbols.

### First libraries

Coarse, since each library costs a fetch, a compile per worker in the browser
and an instantiation per thread:

| Library | Holds |
|---|---|
| runtime (root) | std, allocator, wasi-libc, libc++ |
| gpu | wgpu with the custom backend, block-gpu-guest |
| editor | block-editor-plugin, block-plugin-api, reactive, geometry, serde, serde_json, uuid |
| beui | beui and everything under it, beui-platform-plugin, block-editor-beui, FreeType, HarfBuzz |

Triangle links runtime, gpu and editor. Every beui editor links all four.
**(ask)** whether serde, serde_json and uuid belong in editor or in a separate
small library.

### Rules the linker enforces

- **Each crate and each C library lives in exactly one library of a plugin's
  closure.** Two copies of `reactive` would give two reactive runtimes, which
  fails silently. Buck computes a library's crates as its dependency closure
  minus the closures of the libraries it needs; `wasm-link` refuses a closure
  in which two libraries share a crate or an archive, naming it.
- **Every import resolves to exactly one export in the module's closure.** A
  missing symbol fails the link with its name, as today; a symbol two libraries
  define fails too.
- **Constructors run in dependency order.**

### Thread-locals (do this first, as a spike)

The current scheme cannot extend past a chain. In a chain, each library's
thread-locals would follow those of the libraries below it, all inside the
root's reserve, which becomes a hard cap on everything above the root. In a
diamond (a plugin needs A and B, both need gpu), A and B are linked separately,
both place their thread-locals right after gpu's, and they overlap.

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

Spike: a root, two libraries that both need a third, and a plugin needing both;
a generic in the plugin reaching a thread-local in each library; a spawned
thread doing the same; on wasmtime and in the browser. If neither route works,
**(ask)** whether to ship chains only, with a larger reserve, and refuse
diamonds at link time.

### Loading

`wasm-dylink`'s plan becomes a list of modules in dependency order, each with
its memory, table slots, imports (resolved against its closure), GOT entries,
data relocations and thread-local layout. `dylink.rs` and `dylink.js` loop over
it. A spawned thread instantiates the whole list again. Natively the host keeps
one compiled module per identity per engine; in the browser, cache compiled
modules across workers if the platform allows it.

### Build rules

- `wasm_library(libraries = [...])` in `buck/wasm/defs.bzl` replaces
  `plugin_sdk()`; the root gets its own rule or flag.
- `editor(libraries = [...])` replaces the hard-coded `PLUGIN_SDK`. Pick a
  default (all four) so existing editors' BUCK files need no change, and have
  triangle name runtime, gpu and editor.
- `buck/app/stage.sh` stages every library in every staged plugin's closure.
- `./scripts/buck run //:buckify` keeps working: a library's Cargo.toml lists
  its crates as today's `block-plugin-sdk/Cargo.toml` does.

### Third-party plugins

To compile a Rust plugin against a library, rustc needs that library's exact
rlibs and metadata, not just its `.wasm`. A published library is a bundle: the
module, its rlibs and rmeta, its `be3.lib`, and the toolchain identity. Publish
the bundles from CI next to `bootstrap` (guides/bootstrap.md). **(ask)** where
third-party plugins fetch them from, and whether a plugin package may ship
libraries of its own (C libraries, its own Rust crates) for the host to share
between plugins that name the same identity.

### Performance

Code moved out of the root becomes position-independent and reaches its
globals through the GOT. Measure the beui library against today's SDK with
`app-bench` (guides/performance.md) before landing the split, and record the
result in the PR.

### Tests

- The diamond from the spike, on both hosts, kept as a test.
- A link that puts one crate in two libraries fails, naming the crate.
- A missing symbol fails the link, naming it and the module that imports it.
- A library whose `.wasm` does not hash to its name is refused by both hosts.
- Triangle loads without the beui library staged.
- `//crates/app-tests:test` passes with the four libraries.

### Guides

Rewrite guides/plugin_sdk.md around libraries (its title, "What is in it",
"Identity", "How the links are made", "Thread-locals", "Loading"), and update
the mentions of "the plugin SDK" in guides/adding_a_plugin_editor.md and
guides/buck2.md.

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

Move from `block-editor-beui` into `block-editor-plugin`, or a new crate
beside it **(ask)**, the parts that are state over `EditorHost` rather than
views:
- `Editor`, `OwnedEditor`, `Artifacts`, `Creation` and `ChildState` from
  `editor.rs`, which has no `view!`. It still uses `beui::native::Document`,
  `EmbedSlot` and `CanvasView`; split those uses off into what stays in
  block-editor-beui.
- `fitting.rs` and `related_content.rs`, which use only `beui::reactive` and
  geometry, both of which the lower crates have.
- The data halves of `version_control.rs` and `layered_settings.rs`.

`reactive` stays the state model: an immediate-mode framework reads the same
signals each frame.

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
3. `be3.lib`, the closure checks and the N-module plan in `wasm-link` and
   `wasm-dylink`; both loaders; the build rules. One SDK still, now described as
   a root and one library, to land the machinery without moving code.
4. Split into runtime, gpu, editor and beui. Measure. Move triangle off beui.
5. Fill the GPU gaps, one group per PR.
6. Editor state and the region driver (part 3), except docks and overlays.
7. Publishing library bundles for third parties.
