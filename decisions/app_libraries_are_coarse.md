# Rust apps link two coarse libraries: base and beui

Base holds std, alloc, the allocator, the panic runtime, wasi-libc, libc++ and libc++abi, wgpu with the custom backend, be-wasm-ops, be-sdk-api, be-sdk, geometry, serde, postcard, serde_json and uuid. beui holds `beui-reactive`, beui and everything under it, be-sdk-beui-platform, be-sdk-beui, FreeType and HarfBuzz. An app without beui (the triangle) links base alone, and an app may link no library and be one module written against the raw app interface. Each crate and C library lives in exactly one module of an app's closure, which `be-wasm-link` enforces.

## Why

- Each library costs a fetch, a compile per worker in the browser and an instantiation per thread, so libraries are few; split base further only when a module needs part of it.
- serde, postcard and uuid go in base because be-sdk-api and be-wasm-ops encode every frame with postcard and block ids are uuids.
- `beui-reactive` is beui's, not base's: only a retained, fine-grained framework needs signals, an immediate-mode one rereads its state every frame, and a framework with signals of its own would want its own. So be-sdk keeps plain state with change callbacks and be-sdk-beui adapts them to signals.
- Two copies of a crate such as `beui-reactive` would give two runtimes and fail silently, hence the one-module rule.
