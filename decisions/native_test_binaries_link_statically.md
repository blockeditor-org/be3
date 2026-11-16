# Native test binaries link their Rust dependencies statically

A native `:test-harness` links every Rust crate in its closure into one executable, as every other Rust binary here does. It is not linked against shared libraries to make relinking cheaper.

## Why

Linking is the largest part of rebuilding a big native test binary: in November 2026, from buck2's cached wall times, be-host-stack's `:test-harness` took 24 s to link and be-server's 16 s, longer than compiling be-server's library (9 s), and be-host-stack's is relinked on 29% of edits (plans/cheaper_incremental_builds.md). A test binary linked against a shared object per crate would relink only the crates above an edit, not wasmtime, cranelift, tokio, SQLite and the first-party crates below it. Counting the other big harnesses too, perhaps 15 to 25 s of wall time an edit.

It was not done because the prelude cannot do it for tests alone:

- `link_style = "shared"` on a Rust binary makes only its native (C) dependencies shared. Rust dependencies stay static unless the Rust toolchain sets `advanced_unstable_linking` (`_adjust_link_strategy_for_rust_dependencies` in the prelude's rust/link_info.bzl).
- That is a field of the toolchain, not of a target. With it on, every Rust binary, dylib and cdylib links through the C++ toolchain from rlib objects: be-host, the apps and libraries be-wasm-link links, the release and LTO builds, on every platform.
- It also requires the toolchain to name the standard library's crates (std, core, alloc and the rest) as explicit sysroot dependencies, for every platform built: Linux x86_64 and arm64, wasi and wasi_guest, wasm32, Android, Windows and macOS. Without them analysis fails ("Explicit sysroot deps are required when advanced_unstable_linking is on"), as tried on be-server's test binary.
- A shared object per crate, std included, would then have to reach each test on a worker: buck/dev/test.bzl's `--library-path` and the test's runtime inputs.

So the saving would cost a change to how everything links, for a minority of each edit's cost.

## Reopen when

- the prelude can link one target's Rust dependencies as shared objects without the toolchain-wide setting; or
- the toolchain gets explicit sysroot dependencies for another reason (a custom standard library, build-std), so only the switch and the test launcher's library path remain.

## Since

November 2026: guests now run on V8 (decisions/guests_run_on_v8.md), so wasmtime and Cranelift are no
longer among the crates a test binary links; be-host links rusty_v8's prebuilt static library
instead. The decision stands.
