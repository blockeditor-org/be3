#[cfg(feature = "wgpu")]
mod convert;
#[cfg(test)]
mod tests;

be_ops_bindgen::generate!({
    path: "wit",
    world: "gpu",
    deps: ["../be-ops/wit"],
    with: {"uuid": [u8; 16], "hash": [u8; 32]},
    local: ["be3:gpu/device@0.1.0"],
});
