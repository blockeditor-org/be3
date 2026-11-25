# External textures for apps

Depends on: none.

Vello samples `wgpu::Features::EXTERNAL_TEXTURE` textures (video frames, camera images) without a
copy where the device has the feature (vello_gpu's `wgpu_external_texture`, on in the beui
library). An app cannot make one: crates/be-wasm-gpu-guest's `create_external_texture` is
unimplemented, since be3:gpu carries no external texture. Add one to be3:gpu (a texture the host
imports, and the planes and colour conversion WebGPU's `GPUExternalTexture` names), implement it in
be-wasm-gpu-host and the guest, and let an app's Vello scenes sample it.
