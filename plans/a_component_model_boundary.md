# A component-model boundary: the wasm boundary in WIT, answered as a component's would be

Depends on: plans/be3_gpu_is_wasi_webgpu.md, step 1 (the canonical ABI's lifting and
lowering), for step 1. Step 2 depends on plans/be3_gpu_is_wasi_webgpu.md, step 3b, and
plans/unified_serialization.md, step 4.

decisions/wit_describes_the_wasm_boundary_and_rust_the_frames.md: the wasm boundary is WIT,
bound by wit-bindgen and answered in the component model's canonical ABI, so guests
become real components later (perhaps years from now, once V8 and the browser can run them)
with little change; our own protocols move to Rust, inside the frames
(plans/unified_serialization.md).

The boundary has two parts today. The GPU is moving to wasi:webgpu, bound in the guest by
upstream wit-bindgen and answered by both hosts in the canonical ABI, its async built-ins
included (plans/be3_gpu_is_wasi_webgpu.md, step 1). The rest is the pure-guest ABI: Rust
constants in `crates/be-wasm-guest-abi` (`host_send`, `host_receive`, `host_wake`,
`host_panic`; `app_start`, `app_step`, `app_shutdown`).

## Steps

1. **The raw world in WIT.** `be3:guest`, a versioned package: imports to send and receive
   frames, wake and panic, and the exports a guest's host steps it through, named and typed as
   a component's would be. Guests bind it with wit-bindgen; hosts answer it in the canonical
   ABI with the lifting and lowering the GPU plan's step 1 generates, so a frame the host
   returns is written through the guest's `cabi_realloc`, rather than into a buffer the guest
   guessed the size of and asked again (`host_receive` today). Every host answers it
   (be-wasm-host, the browser's `guest.js` and `app.js`, tabletop-host's wasmi,
   be-wasm-root-host), and be-wasm-guest-abi's constants go.
2. **be-wasm-ops-bindgen goes.** With apps on wasi:webgpu (plans/be3_gpu_is_wasi_webgpu.md,
   3b) and our protocols defined in Rust (plans/unified_serialization.md, step 4), neither its
   `local` mode nor its op mode is left; decisions/guest_calls_are_async_ops_except_local_ones.md
   is rewritten to say how guest calls travel then.
3. **What the component model has that we only imitate**, kept as a list in this plan as it is
   found, built only when the component model can be adopted, so the list says what adoption
   changes:
   - our protocols' async: the be-wasm-ops kernel inside frames, against components' own async
     calls and streams;
   - how a guest's libraries are linked (`dylink.0`) against components' composition;
   - WASI itself: be-wasm-pure-wasi answers preview 1.

## Rules while it is not the component model

- No import or export a component could not have: every function's signature is one a WIT
  world gives.
- We answer the canonical ABI as the spec says (alignment, `cabi_realloc`'s arguments, UTF-8
  strings, async built-ins); where we cannot yet, the difference is in step 3's list.
- Packages are versioned (`@0.1.0`) as WIT's are, and nothing is frozen until a format is
  (guides/frozen_formats.md).
