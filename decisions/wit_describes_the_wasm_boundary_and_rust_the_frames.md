# WIT describes the wasm boundary, and Rust what travels inside it

Not built yet: plans/a_component_model_boundary.md and plans/unified_serialization.md do it.

Two layers, each described one way:

- **The boundary** is WIT: what a guest imports and exports as wasm functions. That is a small
  world of our own (sending and receiving frames, waking, panicking, and the guest's entry
  points) and wasi:webgpu, adopted as published rather than rewritten. Both are answered with
  the component model's canonical ABI, so that, once guests can run as real components (on V8,
  natively and in the browser), they become components with little more than a wrapper.
- **What travels in the frames** is Rust: every protocol of our own (apps, type modules, host
  services, the root, the server) is `#[interface]` traits and `#[derive(Schema)]` types in
  be-wasm-schema's encoding, with ids from `schema.lock`. A description of them for readers in
  other languages is generated, not written.

## Why

- **A standard is worth keeping as it is.** wasi:webgpu's value is that someone else maintains
  it, with tools and guest crates around it; restating it in Rust would mean following its
  changes by hand. The raw world is a handful of functions, and in WIT any language's
  `wit-bindgen` can bind it.
- **Our protocols need what WIT cannot say**: ids that a lockfile holds, must-understand
  members, kept unknown fields, migrations by code, and document semantics. In WIT these would
  be a dialect read from doc comments; in Rust they are attributes, and the stack's own types are
  used directly, with no `with` mapping kept in step by hand.
- **One way per layer**: the two languages never describe the same thing.
- **The component model later.** Matching its ABI and conventions now (canonical-ABI imports,
  `cabi_realloc`, resources as handles, versioned packages, its names for worlds and exports)
  keeps adopting it a change of runtime, not of every guest.

## Rejected

- **WIT for everything**, as today: our protocols' evolution rules would live in a WIT dialect
  no other tool reads.
- **Rust for everything**, the GPU included: we would maintain a copy of wasi:webgpu.
- **Running guests as components now**: V8 does not run components itself, and guests run on
  V8 everywhere (decisions/guests_run_on_v8.md).
