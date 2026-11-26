# A component-model boundary: the wasm boundary in WIT, answered as a component's would be

Depends on: none. Step 3 depends on plans/be3_gpu_is_wasi_webgpu.md, step 3b (apps on
wasi:webgpu and `be3:gpu/device` gone).

decisions/wit_describes_the_wasm_boundary_and_rust_the_frames.md: the wasm boundary is WIT,
answered as closely to the component model's canonical ABI as our runtime allows, so guests
become real components later (perhaps years from now, once V8 and the browser can run them)
with little change; our own protocols move to Rust, inside the frames
(plans/unified_serialization.md).

The boundary has two parts today. The GPU is moving to wasi:webgpu
(plans/be3_gpu_is_wasi_webgpu.md), bound by be-wasm-ops-bindgen's local generator
(`crates/be-wasm-ops-bindgen/src/local.rs`), which passes scalars, borrows, and options of
lists and strings flat, as the canonical ABI does, and lowers async funcs, futures and streams
to tickets settled by one poll, which the canonical ABI does not. The rest is the pure-guest
ABI: Rust constants in `crates/be-wasm-guest-abi` (`host_send`, `host_receive`, `host_wake`,
`host_panic`; `app_start`, `app_step`, `app_shutdown`).

## Steps

1. **The raw world in WIT.** `be3:guest`, a versioned package, bound by the local generator:
   imports to send and receive frames, wake and panic, and the exports a guest's host steps it
   through, named and typed as a component's would be. A frame the host returns is written
   through the guest's `cabi_realloc`, rather than into a buffer the guest guessed the size of
   and asked again (`host_receive` today). Every host answers it (be-wasm-host, the browser's
   `guest.js` and `app.js`, tabletop-host's wasmi, be-wasm-root-host) and be-wasm-guest-abi's
   constants become the generated bindings.
2. **The local generator's synchronous calls follow the canonical ABI throughout.** Where it
   still encodes a parameter or result with postcard (a record that is not flat values, such as
   a canvas configuration), lay it out in memory as the canonical ABI does instead, so a sync
   call is the call a component makes. Its async lowering stays (step 4).
3. **The guest ABI's other half.** With apps on wasi:webgpu and `be3:gpu/device` gone, the local
   interfaces left are wasi:webgpu, `be3:gpu/host` and `be3:guest`; the op-mode generator
   (async funcs as be-wasm-ops ops) goes with our protocols' move to Rust
   (plans/unified_serialization.md, step 4), and decisions/guest_calls_are_async_ops_except_local_ones.md
   is rewritten to say so.
4. **What the component model has that we only imitate**, kept as a list in this plan as it is
   found, built only when the component model can be adopted, so the list says what adoption
   changes:
   - async funcs, `future` and `stream` (WASI 0.3): lowered to tickets and one generated poll,
     not the canonical ABI's async (waitables, `task.return`), so a binary built against our
     bindings runs only on our hosts (decisions/be3_gpu_is_wasi_webgpu.md);
   - the be-wasm-ops kernel inside frames, against components' own async calls;
   - how a guest's libraries are linked (`dylink.0`) against components' composition;
   - WASI itself: be-wasm-pure-wasi answers preview 1.

## Rules while it is not the component model

- No import or export a component could not have: every function's signature is one a WIT
  world gives.
- Where we answer the canonical ABI, we answer it as the spec says (alignment, `cabi_realloc`'s
  arguments, UTF-8 strings); where we cannot yet, the difference is in step 4's list.
- Packages are versioned (`@0.1.0`) as WIT's are, and nothing is frozen until a format is
  (guides/frozen_formats.md).
