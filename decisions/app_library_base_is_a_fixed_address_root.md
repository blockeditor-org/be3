# The base app library is a fixed-address root, not a library like the others

Apps link any set of shared libraries, but base (`//crates/be-sdk-library-base:module`) is the root of every linked app: it keeps its own static data, `__stack_pointer`, heap, table and main-thread thread-local block, and is not position-independent. The loader creates the memory and places everything above it; a link plan is the list [root, libraries..., app], and a standalone app is a list of one.

## Why

Rust's prebuilt std for wasm32-wasip1-threads and both threaded wasi-libc builds use absolute data relocations, which rust-lld refuses in a `-shared` link, so the module holding them cannot be position-independent. The user chose to keep it as the root.

## Rejected

- No special library, with the loader owning memory, table, stack and every thread-local block, as first planned: it needs `-Zbuild-std` and a threaded wasi-libc built with `-fPIC`.
