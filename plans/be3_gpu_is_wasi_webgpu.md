# be3:gpu is wasi:webgpu

Depends on: none.

decisions/be3_gpu_is_wasi_webgpu.md: an app's gpu is wasi:webgpu 0.3.0-rc.2 (pinned and
fetched, never copied into the repository), beside a small `be3:gpu/host` interface for
surfaces and shared images (guides/app_libraries.md, "The gpu"). What is left; each step lands
on its own.

## 1. Taking a later wasi:webgpu

When upstream moves (a new release candidate or a release), change the `http_file`'s URL to
the new commit and its `sha256` and `size_bytes` to the new file's, regenerate, and fix the
host and guest; record the version in decisions/be3_gpu_is_wasi_webgpu.md. Rust that names
the interface's module takes it from `be_wasm_gpu_abi::embed::webgpu::MODULE`; what cannot
names the version itself, and changes with it:

- the WIT that imports or uses it: be-wasm-gpu-abi's `wit/world.wit` and `wit/host.wit`, and
  be-sdk-api's `wit/app.wit`;
- the generator's lists, which are literals: `local` in be-wasm-gpu-abi's `src/lib.rs`, and
  `uses` and `local` in be-sdk-api's `src/lib.rs`;
- the raw apps, which are wat: be-wasm-host's `src/tests/standalone.wat` and
  be-wasm-root-host's `src/tests/painter.wat`.

## 2. Asynchronous pipelines off the app's thread

Compile `create-*-pipeline-async`'s pipelines on a worker thread natively (wgpu's device and
the descriptor's objects are `Send`), and in the shim through the browser's
`createRenderPipelineAsync` and `createComputePipelineAsync` with web-sys, settling through the
poll as now.

## 3. One poll per app, and cancelling

- Each ticketed local interface has its own `[poll]`; an app's executor would rather make one
  call per wake for all of them. wasi:webgpu is the only ticketed interface, so this waits
  until there is a second.
- A dropped `Ticket` or `Items` stops waiting on the guest, but the host keeps its `Promise` or
  `Sender` until it settles; listing dropped tickets in the next poll would let it let them
  go.

## 4. Per-draw calls as cheap as before

A per-draw call on wasi:webgpu costs about 135 ns in release (be-wasm-host's
`example-gpu-calls`), about 12% more than `be3:gpu/device`'s did: each looks up the pass it
records into and the app device that owns it (`Gpu::render`, `checked`, `device_lost`), and
takes the pass out of its table and puts it back. Keeping each pass's device with the pass, and
the pass borrowed in place rather than taken, would bring it back.

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

Wide-gamut and HDR displays: no display the host draws on shows display-p3 or extended range
yet, so be-wasm-gpu-host's `looks.rs` clips such a canvas to sRGB. When one does, its
compositor shows the canvas as it is instead.
