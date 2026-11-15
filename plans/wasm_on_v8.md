# Running every wasm guest on V8 and deleting precompilation

Depends on: none. Within it, step 4 waits for PR #686 to land and step 5 for Android's prebuilt V8
(step 2).

Every app module, type module and library is precompiled to a `.cwasm` for each target
(`:precompiled` in `crates/be-host/BUCK`), one of the slowest actions in a full build and most of
what ships (guides/performance.md, Size). On V8 a module compiles when it loads (Liftoff first,
TurboFan behind it on hot functions) fast enough that nothing is precompiled, and its imports
bind to native functions with no JavaScript between.

## What the prototype found

`prototypes/wasm_on_v8/gpu-calls` (a cargo project outside the workspace; it goes when step 3
lands) runs `be-wasm-host`'s `example-gpu-calls` guest on rusty_v8 (the `v8` crate, 152.2.0):

- **Imports as fast-API calls.** With `--wasm-unsafe-fast-api-wrapper` set before
  `V8::initialize`, a wasm import that is a `FunctionTemplate::build_fast` function with one
  `CFunction` is called straight from wasm code. The `CFunctionInfo` is the receiver
  (`Type::V8Value`), the import's own arguments and `Type::CallbackOptions`, whose `data` is a
  `v8::External` pointing at the guest's host state; `i64` is `Int64Representation::BigInt`. A
  `macro_rules!` over `be_wasm_gpu_abi::gpu_local_imports!` generates one `extern "C"` function
  and `CFunctionInfo` per import, as `link_gpu` does for wasmtime today. The slow callback is never
  taken for these signatures; the prototype's throws if it is.
- **Memory.** A shared `WebAssembly.Memory` is a `SharedArrayBuffer` whose backing store's data
  pointer and length are the guest's memory for `Caller::read`/`write`; it can move only when the
  guest grows it, which a host call sees by reading the length again.
- **Cost.** With the same per-call timing `State::host` does (`Instant` around each call,
  `be_wasm_gpu_host::only_records` aside), per gpu call, release, three runs each:
  wasmtime fastest 101.6/114.7/108.6 ns, mean 118.8/131.3/120.0; V8 fastest 109.4/102.2/108.5,
  mean 119.1/116.4/121.3. Parity: the import costs about 10.5 ns on V8 against 4.5 ns on wasmtime,
  lost under the work each call does. Without the timing V8's call alone measured 41 ns.

Accepted in exchange for deleting precompilation: no fuel and no canonical NaNs on the app host
(plans/host_as_platform_adapter.md's deterministic guests stay on wasmi), the dearer import,
slower cold code until TurboFan, be-host about 36 MB larger, the unsafe flag, and no code cache
on disk. Kept: the memory cap of a pure guest (its memory's maximum clamped, rewriting the memory
section where the module declares a larger one), trap backtraces and `BE3_PERF_MAP`
(`--perf-basic-prof`), stopping a hung step within `STEP_TIMEOUT`, textures never leaving the GPU.

## Steps

1. **The gate.** Done: the numbers above.
2. **The `v8` crate under buck.** Done but for Android. `third-party/v8/BUCK` pins rusty_v8's
   prebuilt static library for each of cross.bzl's platforms by hash and size;
   `third-party/rust/overlays/v8/build.rs` links it and hands the crate the bindings it ships for
   that platform, from OUT_DIR (`overlays/v8/src/binding.rs`), since cargo's script keeps both
   outside OUT_DIR. rusty_v8 publishes nothing for Android: `scripts/internal/build-rusty-v8-android.sh`
   builds both files on a person's machine, and once they are hosted, `android_arm64` and
   `android_arm64_binding` in third-party/v8/BUCK name them, and be-wasm-host's Cargo.toml drops
   its `cfg(not(target_os = "android"))` on `v8`. Until then Android stays on wasmtime.
3. **be-wasm-host on V8.** Its public API stays (`Host`, `App`, `Runnable`, `Interrupter`,
   `Stored`). One isolate per guest thread. Libraries compiled once and shared between isolates
   as `CompiledWasmModule`. The gpu and transport imports as fast-API functions generated from
   `gpu_local_imports!` and `transport_functions!`. The dylink `Plan` carried out with
   `WebAssembly.Global`, `Table`, `Memory` and `Instance`, as `crates/be-host/web/dylink.js` does
   in the browser. WASI preview1 in Rust, as `crates/be-host/web/wasi.js` is in JavaScript.
   `thread-spawn` an OS thread with a new isolate given the same memory and module. Epoch
   interruption becomes `Isolate::terminate_execution` from the `Interrupter`, which also ends
   `memory.atomic.wait`. Reviewed for memory safety (the backing store pointer, the externals'
   lifetimes) as well as correctness.
4. **The other wasmtime users.** `be-block-type-host`, `be-sdk-test-runner` and its
   `wasm_test_harness`, and be-host-runtime's backend. The remapper and the tabletop stay on
   wasmi.
5. **Delete precompilation.** `precompile.rs`, `precompiled_apps`, the precompiler targets, the
   `.cwasm` staging including the APK's assets, and `--quick` tests' precompiles; update
   guides/app_libraries.md, guides/apps/runtime.md, guides/performance.md and
   decisions/host_startup_overlaps_app_loading.md. Only once Android runs on V8.
6. **Measure and decide.** Host start to the files pane, the host tests' CPU, bundle size and
   `example-gpu-calls` before and after, recorded in a decision file.
