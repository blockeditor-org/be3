use super::*;

#[test]
fn a_format_the_compositor_cannot_read_is_refused() {
    let (device, _queue) = vulkan_device();
    let vulkan = Vulkan::of(&device).expect("the device can import dmabufs");
    let dmabuf = memfd_dmabuf(64, 16, Fourcc::Rgb565, &pattern(64, 16));

    assert!(
        vulkan
            .import(&device, &dmabuf, false, Usage::Sample)
            .is_err()
    );
}
