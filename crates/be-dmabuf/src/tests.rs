use super::*;

mod a_format_the_compositor_cannot_read_is_refused;
mod a_linear_dmabuf_imports_with_its_pixels;
mod a_submission_exports_a_fence_that_signals_when_it_finishes;
mod linear_argb_is_advertised_for_sampling;

use crate::testing::{memfd_dmabuf, pattern, read, vulkan_device};
