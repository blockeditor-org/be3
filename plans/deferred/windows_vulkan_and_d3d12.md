# Vulkan and D3D12 on Windows

Depends on: none.

On Windows beui draws through wgpu's Vulkan backend alone (`features = ["vulkan"]` under
`cfg(windows)` in crates/beui-renderer-wgpu/Cargo.toml). D3D12 is the better presenter there:
wgpu answers a resize with `ResizeBuffers` on the same flip-model swapchain rather than
`vkDeviceWaitIdle` and a new swapchain, can present through DirectComposition
(`Dx12SwapchainKind::DxgiFromVisual` in the instance's backend options), and Windows drivers test
it far more than their Vulkan window-system integration, which falls out of step with DWM while a
window resizes.

Vulkan stays because two things on Windows are written against it:

- **XR.** beui-layer-xr opens its OpenXR session through `XR_KHR_vulkan_enable2` and wgpu's Vulkan
  hal, and beui-host-window links it on Windows too. D3D12 needs the session opened through
  `XR_KHR_D3D12_enable` instead, swapchain images wrapped with `create_texture_from_hal::<Dx12>`,
  and the rest of the layer kept free of the hal type.
- **Window capture.** be-host-win32-capture imports each captured window (a shared D3D11 texture
  and fence) into wgpu's Vulkan device through `VK_KHR_external_memory_win32` and
  `VK_KHR_external_semaphore_win32`, which is also why be-host asks for those extensions
  (`OpenDevice::extensions`, `open_vulkan_device`'s Windows arm). On D3D12 the import is
  `ID3D12Device::OpenSharedHandle` for the texture and the fence, then
  `create_texture_from_hal::<Dx12>`: the same API family, so likely simpler.

Supporting both means compiling both backends on Windows, choosing D3D12 for windows unless the
XR layer is asked for (`BEUI_LAYERS`), and keeping each import behind the backend the device was
opened on. Dropping Vulkan on Windows instead needs both items above done for D3D12.
plans/deferred/dxgi_scaling_stretch.md is what D3D12 presentation would still lack.
