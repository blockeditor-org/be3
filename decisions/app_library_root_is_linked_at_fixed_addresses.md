# The root app library is linked at fixed addresses

The module at the root of every linked app closure, the one holding std and wasi-libc (be-sdk-library-base), is linked at fixed addresses and brings its own static data, stack, heap, table and main-thread thread-local block. Every library above it, and every linked app, is position-independent, and the loader places them (guides/app_libraries.md, The root).

## Why

Rust's prebuilt std for wasm32-wasip1-threads and both wasi-libc builds for it (Rust's self-contained one and `third-party/wasi`'s) are compiled with absolute addresses (`R_WASM_MEMORY_ADDR_SLEB`), which rust-lld refuses in a `-shared` link; only wasm32-wasip1's wasi-libc has a position-independent `libc.so`.

## Rejected

- A position-independent root: it needs std built from source (`-Zbuild-std`) and wasi-libc built with `-fPIC` for threads.
