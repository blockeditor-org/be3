# A component-model boundary: our raw world and wasi:webgpu in WIT

Depends on: none.

decisions/wit_describes_the_wasm_boundary_and_rust_the_frames.md: the wasm boundary is WIT,
answered with the component model's canonical ABI, so guests become real components later
(perhaps years from now, once V8 and the browser can run them) with little change; our own
protocols move to Rust, inside the frames (plans/unified_serialization.md).

Today the boundary is the pure-guest ABI: Rust constants in `crates/be-wasm-guest-abi`
(`host_send`, `host_receive`, `host_wake`, `host_panic`; `app_start`, `app_step`,
`app_shutdown`) and the `be3:gpu` world (`crates/be-wasm-gpu-abi/wit`), shaped after
wasi:webgpu but our own, generated as a `local` interface (guides/app_libraries.md, The gpu).

## Steps

1. **The raw world in WIT.** `be3:guest`, a versioned package: imports to send and receive
   frames, wake and panic, and the exports a guest's host steps it through, named and typed as
   a component's would be. Lists and strings cross as the canonical ABI lays them out: a frame
   the host returns is written through the guest's `cabi_realloc`, rather than into a buffer
   the guest guessed the size of and asked again (`host_receive` today). Every host answers it
   (be-wasm-host, the browser's `guest.js` and `app.js`, tabletop-host's wasmi,
   be-wasm-root-host) and every guest library binds it (be-wasm-guest-abi, generated from the
   WIT rather than written).
2. **The GPU on wasi:webgpu as published.** Its WIT vendored unchanged; the host answers its
   imports in the canonical ABI (scalars flattened, lists and strings as pointer and length,
   resources as handles), keeping the fast-API path natively and the shim's in the browser;
   `be-wasm-gpu-guest` becomes wgpu's backend over its bindings, or is replaced by an upstream
   one where one fits. What be3 refuses stays refused (anything wgpu marks `unsafe`, features an
   app did not request). `be3:gpu` and the `local` interfaces go, and
   decisions/guest_calls_are_async_ops_except_local_ones.md is updated.
3. **What the component model has that we only imitate**, listed in this plan as it is found:
   async (WASI 0.3's futures and streams against the be-wasm-ops kernel inside frames), how a
   guest's libraries are linked (`dylink.0` against components' composition), and WASI itself
   (be-wasm-pure-wasi answers preview 1). Nothing is built for these until the component model
   can be adopted; the list says what that adoption will change.

## Rules while it is not the component model

- No import or export a component could not have: every function's signature is one a WIT
  world gives, in the canonical ABI.
- No canonical-ABI behaviour of our own: where we answer the ABI, we answer it as the spec
  says (alignment, `cabi_realloc`'s arguments, string encoding UTF-8), so a component runtime
  later answers it the same.
- Packages are versioned (`@0.1.0`) as WIT's are, and nothing is frozen until a format is
  (guides/frozen_formats.md).
