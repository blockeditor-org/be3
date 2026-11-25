# be3:gpu matches WebGPU

`be3:gpu` (`crates/be-wasm-gpu-abi/wit`) gives an app what JS WebGPU gives a page: every
interface, method, descriptor field, feature, limit and error WebGPU has, with WebGPU's
meaning, and nothing WebGPU lacks except what an app needs from its host (below). The means
is wasi:webgpu itself (decisions/be3_gpu_is_wasi_webgpu.md), so that a component-model WebGPU
could take its place if hosts ship one.

Where the WIT cannot say something the way WebGPU does, it says the same thing another way
and keeps WebGPU's values: a canvas's `viewFormats` is a list even though a record holding a
list is passed encoded rather than as flat wasm values.

What it has beyond WebGPU is only the host's part of an app's life, in `be3:gpu`'s own
interface beside wasi:webgpu: the surfaces the host shows (an explicit `present`, as
wasi:graphics-context had, and `age`), textures the host shares under an image id
(`import-texture`), and the poll through which callbacks reach a wasm app that has no event
loop of its own.

External textures and external image copies are deferred until apps get camera access
(plans/deferred/app_libraries.md, "External textures").

## Why

- An app is untrusted code that may also run in a browser tab. WebGPU is the API the web
  already judged safe to hand untrusted code, with validation, limits, coarsened timestamps
  and bucketed adapter information; matching it inherits that judgment instead of
  re-deriving it per call.
- An app written against WebGPU (through wgpu, Dawn or by hand) should run in be3 without
  learning which parts are missing. A subset makes every library an app links a gamble.
- Keeping WebGPU's names and values, and wasi:webgpu's shape, keeps the door open to drop
  `be3:gpu` for a standard component-model interface once one ships, without changing apps.

## Rejected

- **Trimming it to what our apps use.** That was the state before this decision ("a subset
  of WebGPU"). Each gap surfaced as an `unimplemented!` the first time a new app or library
  reached for it, and the subset had no principle to say what belongs.
- **Diverging for ABI convenience.** `surface-configuration` briefly took
  `view-format: option<...>` so the record stayed flat values for two hand-written test
  apps. The convenience was the ABI's, not the app's, and it made the record unlike
  `GPUCanvasConfiguration`; the test apps pass the encoded record instead.
- **Native wgpu features beyond WebGPU** (polygon modes, conservative rasterization,
  clamp-to-border samplers, binding arrays, push constants as wgpu names them). An app also
  runs in a browser, where none exist, and the host's device asks only for browser features,
  so each would fail on every host anyway. One would be added only behind a feature an app
  checks, and only once WebGPU has it.
- **Exposing `unsafe` wgpu calls** (passthrough shaders, pipeline caches). They skip the
  validation that makes handing a gpu to untrusted code safe.
