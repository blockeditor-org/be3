# Every wasm guest runs on V8, and nothing is precompiled

Apps, their threads, type modules, libraries and the wasm app tests run on V8 through rusty_v8
(`crates/be-wasm-host`), each guest thread on an isolate of its own, with the gpu and transport
imports as fast-API calls. V8 compiles a module when it loads (Liftoff, then TurboFan behind it on
hot code), so no module is precompiled and nothing but `.wasm` is staged. Android stays on it too,
broken until a V8 for it is pinned (plans/wasm_on_v8.md).

## Why

- Precompiling was one of the slowest parts of a build and most of what shipped. A release
  `:native` bundle measured 1,235 MB with precompiled code (807 MB of `.cwasm`) and 497 MB
  without, its `.wasm` the same 265 MB (November 2026).
- One engine on every platform but the browser, and the browser's own engine is V8: the native
  host links and runs guests as `crates/be-host/web/dylink.js` and `wasi.js` do.
- A gpu call costs about what it did. `example-gpu-calls` (release, the same per-call timing on
  both) measured fastest 102 to 115 ns per call and mean 119 to 131 on wasmtime, and fastest 115 to
  134 and mean 128 to 145 on V8 on a shared build server, with the import itself about 10.5 ns
  against 4.5.
- A module that took Cranelift seconds loads at once: the triangle linked against the 22 MB base
  library loads and paints in 0.4 s of CPU, where its library alone took ten seconds to
  precompile.

## What it costs

- be-host is larger: the release binary went from 123 MB to 176 MB of loaded sections
  (162 MB to 232 MB on disk), most of it V8.
- Cold code runs slower until TurboFan reaches it, and there is no code cache on disk, so each
  start compiles again, function by function as each first runs (a module itself compiles in
  milliseconds: the 22 MB base library in 33 ms). The host tests show it: all 139 took 831 s of
  CPU against 397 s on wasmtime's precompiled modules, and the median test's host ran 5.9 s
  against 2.6 (November 2026). Where that time goes is not profiled yet.
- The app host has no fuel and no canonical NaNs; a guest that must be deterministic stays on
  wasmi (plans/host_as_platform_adapter.md).
- `--wasm-unsafe-fast-api-wrapper`, which the fast imports need (crates/be-wasm-host/src/engine.rs
  says why it is safe here).
- No guest may declare garbage-collected types or more than one table: V8 stops delivering a
  termination once an isolate's heap is held at its limit, so a guest able to fill it could not be
  stopped (guides/apps/runtime.md).
- Android has no prebuilt V8 from rusty_v8; ours is built by hand
  (scripts/internal/build-rusty-v8-android.sh) and hosted by us.

## Rejected

- Keeping wasmtime for Android until its V8 exists: the user chose one engine over maintaining two.
- `--liftoff-only` or `--no-wasm-dynamic-tiering`, under which V8 does deliver a termination at
  the heap limit: either changes what every app costs to compile or run, where refusing what can
  fill the heap costs no app anything.
- A code cache on disk: V8 compiles fast enough that the cache's invalidation and its files were
  not worth having back.

Not measured yet: host start to the Files pane on its own.
