# Unstretched frames on D3D12 while a window resizes

Depends on: plans/deferred/windows_vulkan_and_d3d12.md.

wgpu-hal's D3D12 surface (`dx12/mod.rs`, `Surface::configure`) creates every swapchain with
`DXGI_SCALING_STRETCH`, and nothing in `wgpu::SurfaceConfiguration` changes it. While a window is
being resized, DWM shows the last frame presented until one at the new size arrives, and with
stretch scaling it squeezes or stretches that frame to the window's new size, so the whole UI
wobbles during a drag instead of staying put.

What beui wants is `DXGI_SCALING_NONE`: the last frame stays at the window's top-left corner at its
own size, cropped when the window shrinks, as X11 does with north-west gravity
(crates/beui-platform-winit/src/backdrop.rs) and as be-host draws a view whose frame arrives at a
size it was not laid out at (decisions/app_frames_split_into_layout_and_present.md). Where the
window grew, the window's background shows (plans/deferred/winit_issues.md).

This needs wgpu to take the scaling mode, for example as a D3D12 surface option beside its
`Dx12SwapchainKind`, upstreamed, or a patched wgpu-hal until it lands. A DirectComposition
swapchain (`DxgiFromVisual`) takes `DXGI_SCALING_STRETCH` or `NONE` alike, so the option covers
both.
