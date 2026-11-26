# be3:gpu is wasi:webgpu

Depends on: none.

decisions/be3_gpu_is_wasi_webgpu.md: an app's gpu is wasi:webgpu 0.3.0-rc.2 (pinned and
fetched, never copied into the repository), beside a small `be3:gpu/host` interface for
surfaces and shared images (guides/app_libraries.md, "The gpu"). What is left; each step lands
on its own.

## 1. Canvas alpha mode, color space and tone mapping

Composite a surface as its configuration asks: premultiplied alpha, display-p3, and extended
range for rgba16float where the display can show it (else as `standard`, as browsers do). This
is be-host's compositor and the shim's own canvas configuration; beui asks for premultiplied
where it draws translucent surfaces.

## 2. Taking a later wasi:webgpu

When upstream moves (a new release candidate or a release), change the `http_file`'s URL to
the new commit and its `sha256` and `size_bytes` to the new file's, regenerate, and fix the
host and guest; record the version in decisions/be3_gpu_is_wasi_webgpu.md. Nothing else in
the repository names its version.

## 3. Asynchronous pipelines off the app's thread

Compile `create-*-pipeline-async`'s pipelines on a worker thread natively (wgpu's device and
the descriptor's objects are `Send`), and in the shim through the browser's
`createRenderPipelineAsync` and `createComputePipelineAsync` with web-sys, settling through the
poll as now.

## 4. One poll per app, and cancelling

- Each ticketed local interface has its own `[poll]`; an app's executor would rather make one
  call per wake for all of them. wasi:webgpu is the only ticketed interface, so this waits
  until there is a second.
- A dropped `Ticket` or `Items` stops waiting on the guest, but the host keeps its `Promise` or
  `Sender` until it settles; listing dropped tickets in the next poll would let it let them
  go.

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
