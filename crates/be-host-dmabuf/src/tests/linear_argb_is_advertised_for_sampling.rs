use super::*;

#[test]
fn linear_argb_is_advertised_for_sampling() {
    let (device, _queue) = vulkan_device();
    let vulkan = Vulkan::of(&device).expect("the device can import dmabufs");

    assert!(vulkan.formats(Usage::Sample).contains(&Format {
        code: Fourcc::Argb8888,
        modifier: Modifier::Linear,
    }));
}
