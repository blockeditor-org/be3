# A thread-local in another app library is reached through an imported GOT.tls global

Each module of a linked app has its own thread-local block and `__tls_base`. `be-wasm-link` rewrites each TLS relocation against a symbol nothing in the link defines into a read of an imported mutable global `GOT.tls.<symbol>`; only the constant is swapped and the code still adds its own `__tls_base`, so the loader sets the slot per thread, after `__wasm_init_tls`, to the symbol's address on that thread minus the importing module's `__tls_base`.

## Why

- The old scheme, where the app placed each SDK thread-local at the SDK's offset inside a reserve at the end of the SDK's block, works only with one module above the SDK: a chain caps everything above at the lowest module's reserve, and in a diamond two libraries place their thread-locals in the same place.
- LLVM forces the local-exec model on wasm, so there is no TLS model that goes through the GOT.

## Rejected

- Chains only, with a larger reserve, refusing diamonds at link time.
- A rustc/LLVM TLS model for wasm that goes through the GOT: none exists.
