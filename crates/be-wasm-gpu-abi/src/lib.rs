#[cfg(test)]
mod tests;

be_wasm_ops_bindgen::generate!({
    path: "wit",
    world: "gpu",
    deps: ["../be-wasm-ops/wit"],
    with: {"uuid": [u8; 16], "hash": [u8; 32]},
    encodings: {"uuid": ::be_wasm_ops::wire::Bytes, "hash": ::be_wasm_ops::wire::Bytes},
    local: ["be3:gpu/device@0.1.0"],
});
