use super::*;

mod a_format_the_compositor_cannot_read_is_refused;
mod a_linear_dmabuf_imports_with_its_pixels;
mod a_submission_exports_a_fence_that_signals_when_it_finishes;
mod linear_argb_is_advertised_for_sampling;

use crate::testing::{memfd_dmabuf, pattern, read};

fn vulkan_device() -> (wgpu::Device, wgpu::Queue) {
    crate::testing::vulkan_device(beui_renderer_wgpu::present::open_vulkan_device)
}
