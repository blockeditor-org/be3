# Dev builds compile apps at opt-level 0

Outside a release build, apps compile at opt-level 0 and the workspace crates under them,
for wasm, at 1 (`OPT_LEVELS` in `buck/platforms/profile.bzl`); only third-party crates get the app
profile's 2, which their cache hides. Test modules are precompiled with Cranelift's
optimizations off (`--quick`).

## Why

A change under beui recompiles every app, so the app profile's cost is paid on almost every
change.

## Rejected

- Compiling apps at the app profile's opt-level 2 in dev builds: that was a third of the CPU
  a change to `be-lib-geometry` cost `./scripts/verify`.
