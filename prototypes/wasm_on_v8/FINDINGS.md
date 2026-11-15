# Running guests on V8: what the prototype showed

Three standalone cargo projects, not part of the buck build or the workspace:

- `v8/`: embeds V8 through rusty_v8 (the `v8` crate, pinned at 152.2.0, which links Deno's
  prebuilt `librusty_v8` downloaded from its GitHub release) and runs a WAT guest whose imports
  are native Rust functions. No JavaScript source anywhere: modules, memories and instances are
  made by calling V8's `WebAssembly` constructors from Rust.
- `wasmtime/`: the same measurements on wasmtime 48, configured as be-wasm-host's App flavour.
- `guest/`: a Rust CPU benchmark (hashing, a hash map, sorting, float maths) built for
  `wasm32-unknown-unknown`.

Build each with `cargo build --release` in its directory (the guest with
`--target wasm32-unknown-unknown`). Measured in November 2026 on a 4-core agent VM.

## Binding imports to native functions

V8 calls a FunctionTemplate's C function directly from wasm (`ImportCallKind::kWasmToJSFastApi`
in `v8/src/wasm/module-instantiate.cc`) when:

- the process sets `--wasm-unsafe-fast-api-wrapper` before `V8::initialize` (off by default,
  "unsafe" because V8 cannot check the receiver for wasm; ours is ignored);
- the template is built with `FunctionTemplate::builder(slow).build_fast(scope, &[CFunction])`,
  with no signature, and the fast function is the first (only) overload;
- the C signature is the receiver (`Type::V8Value`), then the wasm parameters, then
  `Type::CallbackOptions`; no pointer types; i64 needs `Int64Representation::BigInt`.

`options.data` is the template's data: a `v8::External` pointing at the host state. The slow
callback is never called from wasm; the prototype makes it throw to prove it.

A shared memory never moves (V8 reserves its maximum), so a native function reads guest memory
through a base pointer taken once from the memory's `SharedArrayBuffer` backing store.

## What works

| Check | Result |
|---|---|
| shared imported memory, native imports, every value type | works |
| a native import throws (`callback_scope!` then `throw_exception`) | the guest stops; the host sees the error with a wasm frame in its stack |
| wasm exceptions (`try_table`, `throw`, exnref) | work |
| `IsolateHandle::terminate_execution` from another thread | stops a busy loop in 0.1 ms, and a thread blocked in `memory.atomic.wait` in 0.2 ms |
| a second isolate on another thread | gets the same memory (through `ValueSerializer`, as postMessage moves a shared `WebAssembly.Memory`) and the same compiled code (`CompiledWasmModule`, no recompile); ready in 2.4 to 3.4 ms; atomics wait and notify across them |

## Numbers

Compiling be3's modules, built in buck's default (dev) mode:

| module | V8 lazy (default) | V8 Liftoff, eager | V8 TurboFan, eager | wasmtime Cranelift | `.cwasm` |
|---|---|---|---|---|---|
| checklist, 7.7 MB | 0.01 s | 0.05 s | 0.9 s | 1.3 s | 16.8 MB |
| base, 22.3 MB | 0.05 s | 0.17 s | 5.4 s | 5.8 s | 60.2 MB |
| beui, 63.5 MB | 0.11 s | 0.43 s | 10.8 s | 87 s | 155.8 MB |

The guest benchmark, ms per `run(20)`: wasmtime about 82 to 84; V8 about 81 once tiered up,
107 on its first call (functions compile on first call, hot ones are recompiled by TurboFan in
the background); V8 with `--liftoff-only` about 150.

Per import call: wasm to wasm 0.3 ns, wasmtime `func_wrap` 4.5 ns, V8 fast API 10.5 ns, an
ordinary V8 callback 26 ns.

Size: the stripped prototype binary is 49.6 MB against 13.3 MB for the wasmtime one, so V8 adds
about 36 MB to be-host, against about 595 MB of `.cwasm` in the shipped bundle.

Winch (wasmtime's baseline compiler) refuses to start with wasm exceptions enabled.

## Not tried

Linking a real be3 app (the dylink plan, WASI, the gpu imports), memory caps for pure guests,
a disk code cache (rusty_v8 binds only the streaming compile's caching callback, not
`CompiledWasmModule::Serialize`), and any platform but Linux x86_64.
