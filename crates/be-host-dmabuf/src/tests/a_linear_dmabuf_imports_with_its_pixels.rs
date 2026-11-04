use super::*;

#[test]
fn a_linear_dmabuf_imports_with_its_pixels() {
    let (device, queue) = vulkan_device();
    let vulkan = Vulkan::of(&device).expect("the device can import dmabufs");
    let pixels = pattern(64, 16);
    let dmabuf = memfd_dmabuf(64, 16, Fourcc::Argb8888, &pixels);

    let imported = vulkan
        .import(&device, &dmabuf, false, Usage::Sample)
        .expect("a linear buffer imports");

    assert!(!imported.opaque);
    assert_eq!(imported.texture.format(), wgpu::TextureFormat::Bgra8Unorm);
    assert_eq!(
        read(&device, &queue, &imported.texture),
        pixels,
        "the texture shows the client's pixels, not a cleared image"
    );
}
