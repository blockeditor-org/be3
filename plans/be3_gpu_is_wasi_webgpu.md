# be3:gpu is wasi:webgpu

Depends on: none.

decisions/be3_gpu_is_wasi_webgpu.md: an app's gpu becomes wasi:webgpu 0.3.0-rc.2 (pinned
and fetched, never copied into the repository), beside a small `be3:gpu/host` interface for surfaces and shared images, replacing
`be3:gpu/device` (`crates/be-wasm-gpu-abi/wit/device.wit`). Each step lands on its own.

## 1. The local generator handles what wasi:webgpu uses

`crates/be-wasm-ops-bindgen/src/local.rs` refuses async funcs, futures and streams, and
encodes (postcard) any parameter that is not flat values. wasi:webgpu needs:

- **Async funcs, `future` and `stream` results.** Lower an async func to a call that answers
  a ticket; a `future<T>` result or a `stream<T>` to a handle. The generator makes one poll
  import per local interface, `poll(wait: bool)`, which answers every ticket settled and
  every stream item since the last poll, encoded once per poll rather than per call. Waiting
  blocks where the host can block (natively) until everything asked for has settled; in a
  browser it is the same as not waiting. The host wakes the app when anything settles. On the
  guest these are Rust futures and streams that the poll wakes. This replaces
  `be3:gpu/device`'s hand-written `poll` and `settled` record.
- **Options of lists and strings flat**, as the canonical ABI flattens them (a tag, a pointer,
  a length), so `set-bind-group`'s `option<list<u32>>` and `option<string>` labels cost no
  encoding.
- **`result` returns** answering a scalar on success and an error encoded only when there is
  one (`set-bind-group`, `unmap`, `write-buffer-with-copy`).
- **A `use` of another package's resources** (`be3:gpu/host` uses wasi:webgpu's
  `gpu-canvas-context` and `gpu-texture`), with one handle table for both.
- **Records holding owned resources** (`gpu-canvas-configuration-owned` holds a
  `gpu-device`), **variants of error records** (`create-pipeline-error` and the like), and
  resources built by calls (`record-gpu-pipeline-constant-value`,
  `record-option-gpu-size64`, `gpu-supported-limits`), whichever local.rs lacks.

Test each in be-wasm-ops-bindgen with a small interface, run natively and on the web.

## 2. The host implements wasi:webgpu

Fetch `wit/webgpu.wit` at commit `6a776bada0b66d3dbf9da304a49ff2947ce4e1f8` with a buck2
`http_file` in `crates/be-wasm-gpu-abi/BUCK`, as `buck/cargo/BUCK` fetches
`rust-project.zst`:

```python
http_file(
    name = "webgpu.wit",
    sha256 = "8e34bf2a365bc0bbcdd16d0b30a55f1e5c7b26246728e7e5516f74ff9eb968b7",
    size_bytes = 35321,
    urls = ["https://raw.githubusercontent.com/WebAssembly/wasi-gfx/6a776bada0b66d3dbf9da304a49ff2947ce4e1f8/wit/webgpu.wit"],
)
```

It is one file with no `use` of other packages. be-wasm-gpu-abi's library and test take it as
a `foreign_resources` entry placed at `crates/be-wasm-gpu-abi/wit/deps/webgpu/webgpu.wit`, so
the generator finds it under `wit/deps/` as it would a copy, though none is in the
repository (`.gitignore` that path, for builds outside buck). It is never patched: what it
lacks goes in `be3:gpu/host`. Add `be3:gpu/host` (below), and implement both in
be-wasm-gpu-host beside `be3:gpu/device` (which apps still use until step 3), sharing its
tables, per-app device loss, coarsened timestamps, render bundles recorded at `finish`, and
the per-app checks. What changes:

- `get-gpu` → `request-adapter` answers the host's one adapter; `request-device` takes the
  descriptor, and the host keeps each device's required features and limits and validates
  against them (today only the guest does).
- **Several devices per app**, as WebGPU has them: each has its own objects, lost state and
  error scopes. A context may be configured with any of the app's devices; once that device
  is lost or destroyed, `get-current-texture` fails until the context is configured again.
- Adapter info is coarse, as browsers report it (vendor and architecture buckets, empty
  device and description).
- **Errors** go to the device's own error scopes, not a copy the host keeps:
  `push-error-scope`, `pop-error-scope` and `on-uncaptured-error` are forwarded to the wgpu
  device the app's calls run on. On the web that is the worker's own device; natively it is
  the shared device, whose scopes in wgpu 30 are per thread, and each app runs on its own
  thread, so one app's scopes never see another's errors. A pop settles through the poll
  (on the webgpu backend it resolves later; today's `caught()` in be-wasm-gpu-host's
  `lib.rs` polls the pop once, so there every caught error is lost). The host still wraps
  some calls in a scope of its own, to learn that an object it made is invalid: device
  creates, queue calls, `finish` and pass `end`, never per-draw calls. An error it catches
  that way is raised again into the app's scopes. An uncaptured error with no listener goes
  to the app's log.
- A create that fails answers an invalid object (WebGPU's semantics, as the WIT returns no
  option), whose use is a validation error.
- Async pipelines compile on a worker thread natively. wgpu 30 has no async pipeline
  creation on any backend, so the shim calls the browser's `createRenderPipelineAsync` and
  `createComputePipelineAsync` through web-sys. Both settle through the poll.
- `get-compilation-info` counts UTF-16 code units, as WebGPU does.
- Canvas configuration: the format must be bgra8unorm, rgba8unorm or rgba16float (WebGPU's
  canvas formats); usage and view formats as asked. Alpha mode, color space and tone mapping
  are accepted and composited as opaque, srgb and standard until step 4.

The `be3:gpu/host` interface:

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

Host tests: today's be-wasm-gpu-host tests, run against the new interface, plus error scopes
of each kind caught natively and in the shim's test, uncaptured errors, several devices, and
resize against configure.

## 3a. Apps use wasi:webgpu

- be-wasm-gpu-guest becomes wgpu's custom backend over the wasi:webgpu bindings. What wgpu
  has no call for (async pipelines, `set-label`, `max-draw-count`, compilation hints,
  several devices from one adapter) stays reachable only by calling the bindings; a
  `be_wasm_gpu_guest` function exposes async pipeline creation until wgpu has it.
- **sRGB.** A canvas's format is never sRGB, so:
  - an app configures the linear format (`rgba8unorm` or `bgra8unorm`) with its sRGB twin in
    `view-formats`, and draws through the sRGB view where it wants the gpu to encode
    (Vello, which encodes itself, draws through the linear one);
  - the app protocol's `surface-format` (be-sdk-api `types.wit`) keeps only `rgba8-unorm` and
    `bgra8-unorm`, the formats a host may ask an app's surfaces to be;
  - the host's compositor samples an app's texture through its linear view and writes into
    its own target's linear view, so the sRGB-encoded values pass through unchanged and the
    colours stay as the app drew them (decisions/beui_draws_with_vello.md: blending is in
    sRGB everywhere).
- be-sdk: surfaces through `be3:gpu/host` (`surface`, `resize`, `present`), the format from
  the host's `surface-format`. be-sdk-root's `OwnSurface` is rewritten onto the same calls,
  and plans/host_as_platform_adapter.md's mentions of `be3:gpu`'s surfaces are updated to
  name `be3:gpu/host`.
- be-wasm-gpu-shim answers the same imports in a browser worker.
- **Guests whose memory may move** (pure guests and the standalone raw apps, which own a
  memory that is not shared) get the wasi:webgpu imports made the ordinary way, not as
  fast-API functions (be-wasm-host's `gpu.rs`). The raw apps (be-wasm-host's
  `standalone.wat`, be-wasm-root-host's `painter.wat`) import the new names, and they are
  the tests of that path.

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

## Reading the pinned WIT

The file is not in the repository. To read it, build its target and open the path buck2
prints:

```sh
./scripts/buck build //crates/be-wasm-gpu-abi:webgpu.wit --show-output
```

Step 2 adds this to guides/app_libraries.md ("The gpu").

## Not in wasi:webgpu, and deferred

External textures and `copyExternalImageToTexture` (plans/deferred/app_libraries.md, "External
textures"): when they come, they go in `be3:gpu/host`.
