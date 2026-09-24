use super::*;

use crate::test_client::{read, vulkan_device};

#[test]
fn a_new_gpu_redraws_windows_from_the_buffers_they_still_hold() {
    let (mut harness, _device, _queue) = Harness::with_gpu();
    let (_window, id) = harness.open();
    let (device, queue) = vulkan_device();

    harness.app.replace_gpu(
        device.clone(),
        queue.clone(),
        wgpu::TextureFormat::Bgra8Unorm,
    );

    let surface = harness.app.server().state.surface(id).unwrap();
    let current = harness
        .app
        .textures
        .borrow()
        .get(&surface)
        .expect("the window has a texture on the new device");
    let pixels = read(&device, &queue, current.texture.texture());
    assert!(
        pixels.iter().all(|byte| *byte == 0xff),
        "the window's buffer was uploaded again, not left blank"
    );
    assert!(
        harness
            .app
            .clients()
            .signals(id)
            .unwrap()
            .drawing
            .get_untracked()
            .is_some(),
        "the window has a drawing on the new device"
    );
}
