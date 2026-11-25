# be3:gpu is wasi:webgpu

Depends on: none.

decisions/be3_gpu_is_wasi_webgpu.md: an app's gpu becomes wasi:webgpu 0.3.0-rc.2 (vendored,
pinned), beside a small `be3:gpu` interface for surfaces and shared images, replacing
`be3:gpu/device` (`crates/be-wasm-gpu-abi/wit/device.wit`). Each step lands on its own.

## 1. The local generator handles what wasi:webgpu uses

`crates/be-wasm-ops-bindgen/src/local.rs` refuses async funcs, futures and streams, and
encodes (postcard) any parameter that is not flat values. wasi:webgpu needs:

- **Async funcs, `future` and `stream` results.** Lower an async func to a call that answers
  a ticket; a `future<T>` result or a `stream<T>` to a handle. The generator emits one poll
  import per local interface, which answers every ticket settled and every stream item since
  the last poll, encoded once per poll rather than per call, and the host wakes the app when
  anything settles (as `be3:gpu/device`'s `poll` does by hand today). On the guest these are
  Rust futures and streams that the poll wakes. This replaces the hand-written `settled`
  record and its tickets.
- **Options of lists and strings flat**, as the canonical ABI flattens them (a tag, a pointer,
  a length), so `set-bind-group`'s `option<list<u32>>` and `option<string>` labels cost no
  encoding.
- **`result` returns** answering a scalar on success and an error encoded only when there is
  one (`set-bind-group`, `unmap`, `write-buffer-with-copy`).
- **Records holding owned resources** (`gpu-canvas-configuration-owned` holds a
  `gpu-device`), if not already supported.

Test each in be-wasm-ops-bindgen with a small interface, run natively and on the web.

## 2. The host implements wasi:webgpu

Vendor `wit/webgpu.wit` at commit `6a776bada0b66d3dbf9da304a49ff2947ce4e1f8` under
`crates/be-wasm-gpu-abi/wit/deps/webgpu/`, add the `be3:gpu` interface (below), and implement
both in be-wasm-gpu-host beside `be3:gpu/device` (which apps still use until step 3), sharing
its tables, per-app device loss, coarsened timestamps, render bundles recorded at `finish`,
and the per-app checks. What changes:

- `get-gpu` → `request-adapter` answers the host's one adapter; `request-device` takes the
  descriptor, and the host keeps each device's required features and limits and validates
  against them (today only the guest does). Several live devices per app each get their own
  objects, lost state and error scopes.
- Adapter info is coarse, as browsers report it (vendor and architecture buckets, empty
  device and description).
- Errors: each device's error-scope stack; every call is wrapped in wgpu scopes of all three
  filters and each caught error goes to the innermost scope of its kind, else to
  `on-uncaptured-error`'s stream. An app's errors never reach the shared device's handler.
- A create that fails answers an invalid object (WebGPU's semantics, as the WIT returns no
  option), whose use is a validation error.
- Async pipelines compile on a worker thread natively and through the browser's async
  creation in the shim, settling through the poll.
- `get-compilation-info` counts UTF-16 code units, as WebGPU does.
- Canvas configuration: the format must be bgra8unorm, rgba8unorm or rgba16float; usage and
  view formats as asked. Alpha mode, color space and tone mapping are accepted and composited
  as opaque, srgb and standard until step 4.

The `be3:gpu` interface:

```wit
interface host {
    use wasi:webgpu/webgpu.{gpu-canvas-context, gpu-texture};
    use be3:shared/ids@0.1.0.{image-id, surface-id};
    /// The context of a surface the host shows the app, by the id the host gave it.
    surface: func(id: surface-id) -> gpu-canvas-context;
    /// Shows what was drawn into the context's current texture.
    present: func(context: borrow<gpu-canvas-context>);
    /// How many presents ago the current texture was last drawn: 0 when it never was.
    age: func(context: borrow<gpu-canvas-context>) -> u32;
    /// A texture the host shares under an image id; none when none is shared.
    import-texture: func(image: image-id) -> option<gpu-texture>;
}
```

Host tests: today's be-wasm-gpu-host tests, run against the new interface (each test then
moves in step 3), plus error scopes of each kind and uncaptured errors.

## 3. Apps use wasi:webgpu; be3:gpu/device goes

- be-wasm-gpu-guest becomes wgpu's custom backend over the wasi:webgpu bindings. What wgpu
  has no call for (async pipelines, `set-label`, `max-draw-count`, compilation hints,
  several devices from one adapter) stays reachable only by calling the bindings; a
  `be_wasm_gpu_guest` function exposes async pipeline creation until wgpu has it.
- be-sdk and be-sdk-root: surfaces through `be3:gpu/host`; the surface format from
  `get-preferred-canvas-format`.
- The raw apps (be-wasm-host's `standalone.wat`, be-wasm-root-host's `painter.wat`) import
  the new names.
- be-wasm-gpu-shim answers the same imports in a browser worker.
- Delete `device.wit`, its host implementation and conversions, and `gpu_local_imports!`'s
  old list; update guides/app_libraries.md ("The gpu", "The raw app interface") and
  guides/type_modules.md ("Local interfaces").

## 4. Canvas alpha mode, color space and tone mapping

Composite a surface as its configuration asks: premultiplied alpha, display-p3, and extended
range for rgba16float where the display can show it (else as `standard`, as browsers do). This
is be-host's compositor and the shim's own canvas configuration; beui asks for premultiplied
where it draws translucent surfaces.

## 5. Taking a later wasi:webgpu

When upstream moves (a new release candidate or a release), re-vendor it at the new commit,
regenerate, and fix the host and guest; record the version in
decisions/be3_gpu_is_wasi_webgpu.md. Nothing else in the repository names its version.

## Not in wasi:webgpu, and deferred

External textures and `copyExternalImageToTexture` (plans/deferred/app_libraries.md, "External
textures"): when they come, they go in `be3:gpu/host`.
