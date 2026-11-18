# The base app library is a fixed-address root, not a library like the others

Apps link any set of shared libraries, but base (`//crates/be-sdk-library-base:module`) is the root of every linked app: it keeps its own static data, `__stack_pointer`, heap and table, and is not position-independent. The loader creates the memory and places everything above it; a link plan is the list [root, libraries..., app], and a standalone app is a list of one.

## Why

Rust's prebuilt std and the wasi-libc it carries are not position-independent, and rust-lld refuses their absolute addresses in a `-shared` link, so the module holding them cannot be one. That holds for single-threaded `wasm32-wasip1` as it did for `wasm32-wasip1-threads`, the triple guests were first built for (decisions/guests_are_single_threaded.md): measured in November 2026 on Rust 1.98's `wasm32-wasip1` std, its objects' code carries 998 `R_WASM_MEMORY_ADDR_SLEB` and 547 `R_WASM_MEMORY_ADDR_LEB` relocations, and its self-contained `libc.a` 772 and 1,228, where position-independent code has none (it reaches data through `__memory_base` and `GOT.mem`). The user chose to keep base as the root.

## Rejected

- No special library, with the loader owning memory, table and stack, as first planned: it needs `-Zbuild-std` with `-Crelocation-model=pic` and a wasi-libc built with `-fPIC`. Guests being single-threaded removed the thread-local blocks such a loader would also have owned, not the need for those builds.
- Loading libraries after start: nothing shares a table between threads any more, so it could be done, but nothing asks for it yet (guides/app_libraries.md, Loading).
