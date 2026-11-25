# be3:gpu matches WebGPU

Depends on: none.

decisions/be3_gpu_matches_webgpu.md: `be3:gpu` (`crates/be-wasm-gpu-abi/wit/device.wit`) gives an
app everything JS WebGPU gives a page, shaped after wasi:webgpu. This is what it lacks, audited
against the WebGPU spec, in steps that each land on their own. External textures and
`copyExternalImageToTexture` stay deferred (plans/deferred/app_libraries.md, "External
textures"); everything else here is to build.

Each step changes the WIT, the guest (`be-wasm-gpu-guest`, wgpu's custom backend), the host
(`be-wasm-gpu-host`, which both be-wasm-host natively and `be-wasm-gpu-shim` in a browser run),
the raw apps' `.wat` where a signature they import changes, and adds a host test under
`crates/be-wasm-gpu-host/src/tests/`. Where wgpu has no call for a WebGPU feature, the step
says so: an app on wgpu cannot reach it until wgpu does, but the ABI has it for apps (or
libraries) that call `be3:gpu` some other way, and the guest answers it when wgpu grows it.

Already as WebGPU (no step): buffers and mapping, textures and views, samplers' WebGPU fields,
bind groups and layouts, pipeline layouts and `auto` layouts with `get-bind-group-layout`,
render and compute pipelines (synchronous), render and compute passes, render bundles,
copies and `clear-buffer`, `write-buffer`/`write-texture`, timestamp and occlusion queries with
timestamps coarsened to 100 µs as browsers do, `on-submitted-work-done`, device loss and its
reasons, debug markers on encoders and passes, and `get-compilation-info`.

## 1. Errors: scopes, kinds and uncaptured errors

Gap: the ABI has only `take-error`, the first error since it was last taken, as a string. The
guest files every error as a validation error, so `push_error_scope(OutOfMemory | Internal)`
never catches anything; it takes errors only while a scope is pushed, so with none pushed
`on_uncaptured_error` never hears of an error a call (rather than a create) met. The host wraps
each call in a wgpu validation scope only, so an app's out-of-memory or internal error goes to
the shared device's own handler rather than the app.

- WIT: `variant gpu-error { validation(string), out-of-memory(string), internal(string) }`,
  `enum error-filter`, `push-error-scope: func(filter)`, `pop-error-scope: func() -> u64`
  answering a ticket, and `settled` gains `scopes: list<tuple<u64, option<gpu-error>>>` and
  `uncaptured: list<gpu-error>`. `take-error` goes.
- Host: keeps each app's scope stack; wraps a call in all three wgpu filters (or one
  scope per filter it has seen pushed) and files each caught error into the app's innermost
  scope of its kind, else into `uncaptured`. A pop of an empty stack is an operation error,
  as WebGPU rejects it. A create that fails still answers `none` (WebGPU's invalid object).
- Guest: `push_error_scope`/`pop_error_scope` call through; `pop_error_scope`'s future waits on
  its ticket; `settle` hands `uncaptured` to the handler, and with none set logs it (WebGPU
  only warns), never panics.
- Test: an out-of-memory buffer caught by an out-of-memory scope and not by a validation
  scope; an error with no scope reaching the uncaptured list.

## 2. `request-device` takes the app's descriptor

Gap: `request-device: func()` takes nothing, so only the guest knows the features and limits
the app asked for; the host validates against its own device, and an app on another guest
(hand-written, another language) gets the host's whole device.

- WIT: `request-device: func(descriptor: device-descriptor) -> result<_, string>` with
  `label`, `required-features: list<feature-name>`, `required-limits` (each limit optional,
  WebGPU's record of overrides) and `default-queue: queue-descriptor { label }`. Refused as
  WebGPU's `requestDevice` rejects (a feature or limit the adapter lacks).
- Host: keeps the app's features and limits and validates creates against them (formats,
  query types, WGSL `enable`s, every limit), as `features.rs` does in the guest today; then the
  guide's "only the guest validates" line goes.

## 3. Adapter info and GPU-wide queries

Gap: the guest makes up `adapter_info` ("BE3 app host", vendor 0). WebGPU has
`GPUAdapterInfo` (`vendor`, `architecture`, `device`, `description`, `subgroupMinSize`,
`subgroupMaxSize`, `isFallbackAdapter`), `navigator.gpu.getPreferredCanvasFormat()` and
`navigator.gpu.wgslLanguageFeatures`.

- WIT: `adapter-info: func() -> adapter-info`, `preferred-canvas-format: func() ->
  texture-format`, `wgsl-language-features: func() -> list<string>`.
- Host: answers as browsers do: vendor and architecture coarse (the buckets Chrome reports),
  `device` and `description` empty, the subgroup sizes, `is-fallback-adapter` from the
  adapter type. The preferred format is the one the host composites cheapest (bgra8unorm or
  rgba8unorm). WGSL language features are naga's set (`wgpu::Instance::wgsl_language_features`
  where wgpu has it).
- Guest: `adapter_info` maps them onto `wgpu::AdapterInfo`; be-sdk picks the surface format
  from `preferred-canvas-format`.

## 4. Features, limits and formats WebGPU has added

Gaps, by area:

- `feature-name` lacks `core-features-and-limits` (every be3 device is core, so the host
  always offers it, and a compatibility-mode request gets a core device, as the spec allows),
  `subgroups`, `texture-formats-tier1`, `texture-formats-tier2` and
  `texture-component-swizzle`.
- `supported-limits` lacks `max-bind-groups-plus-vertex-buffers`,
  `max-storage-buffers-in-vertex-stage`, `max-storage-textures-in-vertex-stage`,
  `max-storage-buffers-in-fragment-stage` and `max-storage-textures-in-fragment-stage`.
- `texture-format` lacks the 16-bit normalized formats `r16-unorm`, `r16-snorm`, `rg16-unorm`,
  `rg16-snorm`, `rgba16-unorm` and `rgba16-snorm` (`texture-formats-tier1`).
- `vertex-format` lacks the single-component formats (`uint8`, `sint8`, `unorm8`, `snorm8`,
  `uint16`, `sint16`, `unorm16`, `snorm16`, `float16`), `unorm10-10-10-2` and
  `unorm8x4-bgra`.
- `texture-view-descriptor` lacks `swizzle` (`texture-component-swizzle`), if wgpu has it by
  then; otherwise the field and feature land and the host refuses a non-identity swizzle.

Each is a WIT addition, a conversion on both sides and a host test that the feature is offered
only when the host's device has it. Keep this step current with the spec: a feature, limit or
format WebGPU adds later belongs here.

## 5. Drop what WebGPU does not have

`be3:gpu` carries wgpu-native fields that no host can honour, since the host's device asks only
for browser features (`Features::all_webgpu_mask()`): `address-mode.clamp-to-border`,
`sampler-descriptor.border-color` (and `sampler-border-color`), `primitive-state.polygon-mode`
and `conservative`, `bind-group-layout-entry.count` (binding arrays), and a 64-bit
`multisample-state.mask` (WebGPU's `GPUSampleMask` is 32 bits). Remove them; the guest
refuses a wgpu descriptor that sets them with an error naming the field, as it does other
native-only calls.

## 6. Canvas configuration

Gap: `surface-configuration` has `width`, `height`, `format` and `view-formats`.
`GPUCanvasConfiguration` also has `usage`, `alphaMode` (`opaque`, `premultiplied`),
`colorSpace` (`srgb`, `display-p3`) and `toneMapping` (`standard`, `extended`), allows only
`bgra8unorm`, `rgba8unorm` and `rgba16float`, and the context has `unconfigure` and
`getConfiguration`.

- WIT: add `usage: u32`, `alpha-mode`, `color-space` and `tone-mapping` to the record;
  `surface.unconfigure` and `surface.configuration: func() -> option<surface-configuration>`.
- Host: refuses other formats and usages the surface cannot have; makes the surface's
  textures with the asked usages (on top of what compositing needs); composites them as
  opaque or premultiplied, converting display-p3 and extended-range rgba16float content into
  the display's space (be-host's compositor and the shim's canvas configuration, whose own
  `alpha_mode` and `color_space` it then sets from the app's). Where the display cannot show
  extended range, `extended` behaves as `standard`, as browsers do.
- be-sdk: passes `opaque` and `srgb` as today; beui asks for premultiplied where it draws
  translucent surfaces.

`present` and `age` stay: they are the host's part (wasi:graphics-context presents
explicitly too).

## 7. Asynchronous pipeline creation

Gap: WebGPU has `createRenderPipelineAsync` and `createComputePipelineAsync`, whose failure is a
`GPUPipelineError` (`validation` or `internal`) rather than an error in a scope. wgpu has no
asynchronous pipeline creation, so apps on wgpu cannot reach this until it does.

- WIT: `create-render-pipeline-async` and `create-compute-pipeline-async` answering tickets;
  `settled` gains `pipelines: list<tuple<u64, result<u32, pipeline-error>>>` naming the made
  pipeline's handle.
- Host: natively compiles on a worker thread and settles in a later poll, waking the app; in
  the shim it calls the browser's async creation through `web-sys`.
- Guest: a `be_wasm_gpu_guest` function returning a future of a `wgpu::RenderPipeline` until
  wgpu has the call.

## 8. Labels and debug groups everywhere WebGPU has them

Gaps:

- `command-encoder.finish` takes no `GPUCommandBufferDescriptor`, so a command buffer has no
  label.
- `render-bundle-encoder` lacks `insert-debug-marker`, `push-debug-group` and
  `pop-debug-group` (WebGPU's `GPUDebugCommandsMixin` covers bundle encoders).
- `GPUObjectBase.label` is writable; no `be3:gpu` resource can be relabelled (wasi:webgpu has a
  `label`/`set-label` pair on every resource). wgpu has no setter, so the guest cannot reach
  it from wgpu; the ABI gets them for completeness.
- `query-set` has no `destroy`; the guest's is a no-op until the app drops it.

## 9. Compilation info and shader hints as WebGPU counts them

- `source-location` counts bytes of UTF-8 (the WIT says so). `GPUCompilationMessage`'s
  `lineNum`, `linePos`, `offset` and `length` count UTF-16 code units; the host converts.
- `shader-module-descriptor` lacks `compilationHints` (an entry point and its layout). The
  host may ignore them, as the spec allows, but takes them.

## 10. Render pass `maxDrawCount`

`render-pass-descriptor` lacks `max-draw-count`. The host validates draws against it (an
error in the pass once exceeded); wgpu has no field, so the guest cannot pass one until it
does.

## 11. Several devices

JS can request several devices from one adapter, and keep each until it destroys it;
`be3:gpu` has one device per app, its objects free functions. Make the device and its queue
resources (`gpu-device`, `gpu-queue`, wasi:webgpu's shape), `request-device` answering one,
each create a method of it, and the host keep a table of each app device's objects, its lost
state and its error scopes. Land it last: it renames every create, so the steps above go
first, and it also brings the names closest to wasi:webgpu. A device per guest on the host
(plans/deferred/gpu_devices_per_guest.md) is a different matter and stays deferred.

## Kept as they are

- **Compatibility mode** (`featureLevel: "compatibility"`, `textureBindingViewDimension`):
  every be3 device is core, which a compatibility request may get (step 4).
- **Immediates** (`setImmediates`, `var<immediate>`) and other proposals: added in step 4's
  fashion when they reach the spec and browsers ship them; the guest's `set_immediates` is
  `unimplemented!` until then.
- **XR** (`xrCompatible`, WebXR's GPU binding): apps reach a headset through their host, not
  through `be3:gpu`.
