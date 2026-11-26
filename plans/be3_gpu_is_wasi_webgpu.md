# be3:gpu is wasi:webgpu

Depends on: none.

decisions/be3_gpu_is_wasi_webgpu.md: an app's gpu becomes wasi:webgpu 0.3.0-rc.2 (pinned
and fetched, never copied into the repository), beside a small `be3:gpu/host` interface for surfaces and shared images, replacing
`be3:gpu/device` (`crates/be-wasm-gpu-abi/wit/device.wit`). Each step lands on its own.

## 1. The local generator handles what wasi:webgpu uses (landed)

be-wasm-ops-bindgen's local interfaces take async funcs, futures and streams (`u32` tickets
settled through the generated `[poll]` import, `settle(wait)` on the guest), options of lists
and strings flat, fallible returns, calls past 16 arguments and another package's resources
(guides/type_modules.md, "Local interfaces"; crates/be-wasm-ops-local-test). Left for later
steps:

- **The guest half on a real host.** be-wasm-ops-local-test drives the host glue natively and
  the guest's `Settling` directly; a wasm guest calling a ticket func, a fallible func and an
  option-of-list func and polling, run by be-wasm-host on V8, needs an interface the host binds
  with tickets, which only wasi:webgpu is: it is a test of step 2
  (`a_guest_drives_wasi_webgpu_through_its_generated_bindings`). The web leg is step 3a's
  `:standalone-web-test`, whose raw app waits for its adapter and device through `[poll]` in
  the shim.
- **One poll per app.** Each ticketed interface has its own `[poll]`; an app's executor would
  rather make one call per wake for all of them. wasi:webgpu is the only ticketed interface,
  so this waits until there is a second, and the shared handle table step 2 gives them.
- **Cancelling.** A dropped `Ticket` or `Items` stops waiting on the guest, but the host keeps
  its `Promise` or `Sender` until it settles; listing dropped tickets in the next poll would let
  it let them go.

## 2. The host implements wasi:webgpu (landed)

be-wasm-gpu-abi's world imports wasi:webgpu, fetched by buck2 (`:webgpu.wit`), and
`be3:gpu/host` beside `be3:gpu/device`; be-wasm-gpu-host answers both (`src/webgpu.rs`),
sharing `Gpu`'s tables, and be-wasm-host and the shim bind their imports. What it does
differently from the plan as first written:

- **Errors** stay in the host's own copy of each device's scopes rather than wgpu's: the
  host's scopes around creates, queue calls, `finish` and pass `end` catch every error a call
  can raise (per-draw calls raise theirs at the pass's end), and wgpu's device-wide uncaptured
  handler is shared by every app natively. A caught error goes to the scope that was innermost
  when the call was made, once its pop resolves (later, in a browser); an app's pop waits for
  every such call.
- **Async pipelines** are made at once and settled at the next poll, as `GPUPipelineError`s
  when they fail. Compiling on a worker thread natively, and the browser's
  `createRenderPipelineAsync` through web-sys in the shim (wgpu 30 has neither), is step 6.
- wgpu 30 has no `texture-formats-tier1`, `texture-formats-tier2` or
  `texture-component-swizzle`, so no host offers them; a swizzled view is refused. Adapter
  info names nothing; WGSL language features are naga's.
- Objects remember the app device they were made on, so their errors reach its scopes and
  destroying a device destroys its buffers and textures.

The `be3:gpu/host` interface (`crates/be-wasm-gpu-abi/wit/host.wit`):

```wit
interface host {
    use wasi:webgpu/webgpu.{gpu-canvas-context, gpu-texture};
    use be3:shared/ids@0.1.0.{image-id, surface-id};
    /// The context of a surface the host shows the app, by the id the host gave it.
    surface: func(id: surface-id) -> gpu-canvas-context;
    /// Sets the size of the context's textures, as a canvas's width and height do.
    resize: func(context: borrow<gpu-canvas-context>, width: u32, height: u32);
    /// Shows what was drawn into the context's current texture and ends the frame.
    present: func(context: borrow<gpu-canvas-context>);
    /// How many presents ago the current texture was last drawn: 0 when it never was.
    age: func(context: borrow<gpu-canvas-context>) -> u32;
    /// A texture the host shares under an image id; none when none is shared.
    import-texture: func(image: image-id) -> option<gpu-texture>;
}
```

- **Size** stays the app's choice, as a canvas's `width` and `height` are a page's, since
  `gpu-canvas-configuration` has none. `resize` may come before or after `configure`; a
  context has textures once it has both a size and a configuration, and a resize or a
  configure that changes nothing keeps them. A size of zero, or past
  `max-texture-dimension2-d`, is a validation error on the configured device, and
  `get-current-texture` then fails.
- **Frames:** `get-current-texture` answers the same texture, as a fresh handle, until
  `present` ends the frame; the next call starts a new one. A resize or configure mid-frame
  ends it unpresented, as a canvas's does.

Host tests: be-wasm-gpu-host's `*webgpu*` tests (scopes, uncaptured errors, several devices,
canvases, maps and work); be-wasm-gpu-abi's
`wasi_webgpus_calls_made_per_draw_take_only_wasm_values`; be-wasm-host's
`a_guest_drives_wasi_webgpu_through_its_generated_bindings`, which runs
crates/be-wasm-guest-webgpu on V8: step 1's generated guest half (tickets through `[poll]`, a
fallible call's error through `[returned]`, options of lists) against this host. be-wasm-gpu-abi
generating and compiling the fetched WIT is the compile check of both. Today's device tests
move in step 3b.

## 3a. Apps use wasi:webgpu (landed)

- be-wasm-gpu-guest is wgpu's custom backend over the wasi:webgpu bindings, and its surfaces
  are `be3:gpu/host`'s. What wgpu has no call for (`set-label`, `max-draw-count`,
  compilation hints, several devices from one adapter) stays reachable only by calling the
  bindings; `be_wasm_gpu_guest::create_render_pipeline_async` and
  `create_compute_pipeline_async` make a pipeline through the async calls and hand it to
  the next `create_*_pipeline`, since wgpu cannot wrap a custom pipeline itself. The guest
  polls only while a ticket waits, since `[poll]` reaches the gpu and an app whose surfaces
  are not rendered has none.
- **sRGB.** A canvas is never sRGB. The app protocol's `surface-format` keeps only
  `rgba8-unorm` and `bgra8-unorm`, beui's renderers write sRGB-encoded values into the
  canvas, and the host composites them unchanged (decisions/beui_draws_with_vello.md). An
  app may add the canvas's sRGB twin to `view-formats` to have the gpu encode, but be-sdk
  asks for none: nothing it draws needs it, and a host on WebGL cannot view a texture in
  another format at all (wgpu's `VIEW_FORMATS` downlevel flag).
- be-sdk and be-sdk-root's `OwnSurface` draw through `be3:gpu/host`; the raw apps
  (`standalone.wat`, `painter.wat`) import the new names, ask for a device through `[poll]`,
  and are the tests of the ordinary (non-fast) imports guests whose memory may move get.
- A canvas configuration the host's device cannot make (an sRGB twin on WebGL) is refused as
  a validation error, and the canvas keeps no textures.

## 3b. be3:gpu/device goes

Delete `device.wit`, its host implementation and conversions, and `gpu_local_imports!`'s old
list; update guides/app_libraries.md ("The gpu", "The raw app interface") and
guides/type_modules.md ("Local interfaces").

## 4. Canvas alpha mode, color space and tone mapping

Composite a surface as its configuration asks: premultiplied alpha, display-p3, and extended
range for rgba16float where the display can show it (else as `standard`, as browsers do). This
is be-host's compositor and the shim's own canvas configuration; beui asks for premultiplied
where it draws translucent surfaces.

## 5. Taking a later wasi:webgpu

When upstream moves (a new release candidate or a release), change the `http_file`'s URL to
the new commit and its `sha256` and `size_bytes` to the new file's, regenerate, and fix the
host and guest; record the version in decisions/be3_gpu_is_wasi_webgpu.md. Nothing else in
the repository names its version.

## 6. Asynchronous pipelines off the app's thread

Compile `create-*-pipeline-async`'s pipelines on a worker thread natively (wgpu's device and
the descriptor's objects are `Send`), and in the shim through the browser's
`createRenderPipelineAsync` and `createComputePipelineAsync` with web-sys, settling through the
poll as now.

## Reading the pinned WIT

The file is not in the repository. To read it, build its target and open the path buck2
prints:

```sh
./scripts/buck build //crates/be-wasm-gpu-abi:webgpu.wit --show-output
```

guides/app_libraries.md ("The gpu") says the same.

## Not in wasi:webgpu, and deferred

External textures and `copyExternalImageToTexture` (plans/deferred/app_libraries.md, "External
textures"): when they come, they go in `be3:gpu/host`.
