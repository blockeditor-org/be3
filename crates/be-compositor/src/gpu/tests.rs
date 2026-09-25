use super::*;

mod a_format_the_compositor_cannot_read_is_refused;
mod a_linear_dmabuf_imports_with_its_pixels;
mod linear_argb_is_advertised_for_sampling;

use std::io::Write as _;

pub(crate) use crate::test_client::{pattern, read, vulkan_device};

use smithay::backend::allocator::dmabuf::DmabufFlags;

pub(crate) fn memfd_dmabuf(width: u32, height: u32, code: Fourcc, pixels: &[u8]) -> Dmabuf {
    let fd =
        rustix::fs::memfd_create("dmabuf", rustix::fs::MemfdFlags::CLOEXEC).expect("a memfd opens");
    let mut file = std::fs::File::from(fd);
    file.write_all(pixels).expect("the pixels are written");
    let mut builder = Dmabuf::builder(
        (width as i32, height as i32),
        code,
        Modifier::Linear,
        DmabufFlags::empty(),
    );
    builder.add_plane(file.into(), 0, 0, width * 4);
    builder.build().expect("the dmabuf builds")
}
