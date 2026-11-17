//! The sandbox API for modules from bytes (plans/host_as_platform_adapter.md, The sandbox API):
//! the WIT world `be3:sandbox/spawner` and the Rust generated from it, which a guest calls through
//! `guest::sandbox` and a host serves from `embed::sandbox::Call`
//! (crates/be-wasm-sandbox-host natively).

pub use be_wasm_ops as ops;

be_wasm_ops_bindgen::generate!({
    path: "wit",
    world: "spawner",
    deps: ["../be-wasm-ops/wit"],
    with: {
        "hash": [u8; 32],
        "module-id": [u8; 32],
    },
    copy: ["hash", "module-id"],
});
