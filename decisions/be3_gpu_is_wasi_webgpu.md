# be3:gpu is wasi:webgpu

An app's gpu is `wasi:webgpu` (github.com/WebAssembly/wasi-gfx, `wit/webgpu.wit`), vendored
at a pinned version, beside one small `be3:gpu` interface for what an app needs from its
host and wasi:webgpu leaves out. It replaces `be3:gpu/device`, the hand-written interface
shaped after it. It is the means to decisions/be3_gpu_matches_webgpu.md: an app gets what JS
WebGPU gives a page. plans/be3_gpu_is_wasi_webgpu.md is the work.

The pinned version is 0.3.0-rc.2 (commit `6a776bada0b66d3dbf9da304a49ff2947ce4e1f8`).
Moving to a later one is a deliberate change, made like any other ABI change.

The `be3:gpu` interface beside it holds:

- the host's surfaces: a `gpu-canvas-context` for a surface id, with `resize`, `present` and
  `age` (wasi:webgpu has the context resource but no way to get one, and its configuration
  has no size);
- `import-texture`, a `gpu-texture` for an image the host shares under an image id.

`take-error`, `poll`'s hand-written `settled` record and the free functions go: errors are
wasi:webgpu's error scopes and uncaptured-error stream, and its async funcs, futures and
streams settle through the one poll our binding generator gives every local interface.

## Why

- **It covers WebGPU.** It is generated from the spec's WebIDL (`generate/`, webidl2wit) and
  was regenerated against the spec in 2026. It has everything the gap audit of
  `be3:gpu/device` found missing: error scopes, their kinds and uncaptured errors, the
  adapter and its info, the device descriptor, the preferred canvas format and WGSL language
  features, async pipelines, every feature, limit and format up to `subgroups`,
  `texture-formats-tier1/2`, `texture-component-swizzle`, `max-draw-count`, compilation
  hints, labels on every object, full canvas configuration, and several devices. Hand-keeping
  our own copy current would redo that generator by hand, and fall behind.
- **It is the standard we would move to.** Its funcs, records and resources are the ones a
  component-model host would ship, so a later switch changes our binding generation, not
  apps.
- **Its shape costs no more per draw.** Per-draw calls (`draw`, `set-vertex-buffer`,
  `set-pipeline`) take scalars, borrows and options of them, which our `local` generator
  already passes as flat wasm values. `set-bind-group` takes `option<list<u32>>` and returns
  a `result`; the generator learns to pass both without encoding (plan step 1). What is
  shaped differently (limits and features as resources with a getter each, pipeline
  constants as a resource) runs only when an app starts or creates a pipeline.

## What it costs

- It is a Phase 2 proposal at a release candidate, and it has had breaking changes in 2026
  (it dropped graphics-context and surface, and moved to WASI 0.3's async). Pinning a vendored
  copy keeps that churn out until we choose to take it.
- It uses WASI 0.3's `async` funcs, `future` and `stream`, which our runtime (V8, no
  component model) does not have. Our `local` binding generator lowers them to tickets
  settled by one generated poll, which is not the canonical ABI's lowering: a binary built
  against our bindings still runs only on our hosts. The WIT, not the binary, is what is
  shared.
- It has no `copyExternalImageToTexture` or external textures. Both stay deferred
  (plans/deferred/app_libraries.md, "External textures"), and when they come they belong in
  the `be3:gpu` interface beside it.

## Rejected

- **Keeping `be3:gpu/device` and filling its gaps** (the plan this replaces). It would end
  as a hand-written copy of wasi:webgpu with different names, kept current with the spec by
  hand.
- **Running it under the component model** (wasmtime or jco transpiling). Apps run on V8
  natively and in browser workers, with the gpu answered on the app's own thread by fast
  host calls; a component runtime adds a layer to every per-draw call and does not exist
  for a browser worker.
- **Editing the vendored WIT** (dropping the record resources, adding a canvas constructor,
  making async funcs sync). What wasi:webgpu lacks goes in our own interface beside it,
  so the vendored file stays the upstream file.
