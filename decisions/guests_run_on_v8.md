# Every wasm guest runs on V8, and nothing is precompiled

Apps, their threads, type modules, libraries and the wasm app tests run on V8 through rusty_v8
(`crates/be-wasm-host`), each guest thread on an isolate of its own, with the gpu and transport
imports as fast-API calls. V8 compiles a module when it loads (Liftoff, then TurboFan behind it on
hot code), so no module is precompiled and nothing but `.wasm` is staged. Android runs it too, on
a V8 we build (third-party/v8/BUCK).

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

- A code cache entry carries what was left of each function's tiering budget, which V8 restores
  as it loads; left as the session spent it, every later start optimised what that session found
  hot (the host tests took 1,000 s of CPU), so entries are written with fresh budgets.
- be-host is larger: the release binary went from 123 MB to 176 MB of loaded sections
  (162 MB to 232 MB on disk), most of it V8.
- Starts cost more. V8 compiles each function as it first runs, with Liftoff, and optimises hot
  ones with TurboFan behind it (a module itself compiles in milliseconds: the 22 MB base library
  in 33 ms). The user accepted these numbers (November 2026, the 141 host tests on the build
  server; start is the host's launch to the Files pane, the median of 151 starts):

  | | wasmtime, precompiled | V8 | V8, exports trimmed | V8, trimmed, code cache |
  |---|---|---|---|---|
  | Host tests' CPU | 397 s | 831 s | 748 s | 756 s |
  | Start to the Files | 0.56 s | 2.23 s | 1.77 s | 1.67 s |

  Trimming: each library keeps only the exports the staged apps import (plus `__` names and the
  host's entry points), since V8 makes a wrapper for every export as it instantiates, and the
  libraries export about 130,000 names. The code cache keeps V8's TurboFan code on the device
  (crates/be-wasm-host/src/cache.rs), and the host tests read one their build makes by running
  each host; it saves the start little because V8 serializes only TurboFan code, and most of a
  start runs code that never reached it.
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
- Filling the code cache ahead of time with `--no-liftoff`: V8 refuses an entry written under
  other flags, and compiling everything with TurboFan costs 15 s of CPU at start.
- Trimming each app's libraries to that app's own imports: the libraries are then no longer
  shared between apps, and starts cost more than with nothing trimmed.

