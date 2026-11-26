#[cfg(test)]
mod tests;

be_wasm_ops_bindgen::generate!({
    path: "wit",
    world: "gpu",
    deps: ["../be-wasm-ops/wit"],
    with: {"uuid": [u8; 16], "hash": [u8; 32]},
    encodings: {"uuid": ::be_wasm_ops::wire::Bytes, "hash": ::be_wasm_ops::wire::Bytes},
    local: [
        "be3:gpu/device@0.1.0",
        "wasi:webgpu/webgpu@0.3.0-rc.2",
        "be3:gpu/host@0.1.0",
    ],
});

thread_local! {
    static APP_DEVICE: core::cell::Cell<Option<webgpu::webgpu::GpuDevice>> =
        const { core::cell::Cell::new(None) };
}

/// The device an app's surfaces are configured with: the one its wgpu backend
/// (be-wasm-gpu-guest) or a root drawing through these bindings itself requested last.
pub fn app_device() -> Option<webgpu::webgpu::GpuDevice> {
    APP_DEVICE.with(core::cell::Cell::get)
}

pub fn set_app_device(device: webgpu::webgpu::GpuDevice) {
    APP_DEVICE.with(|current| current.set(Some(device)));
}
