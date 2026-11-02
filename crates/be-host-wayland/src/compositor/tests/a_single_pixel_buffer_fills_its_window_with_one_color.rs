use super::*;

use crate::test_client::read;

#[test]
fn a_single_pixel_buffer_fills_its_window_with_one_color() {
    let (mut harness, device, queue) = Harness::with_gpu();
    let (window, id) = harness.open();

    let buffer = harness
        .client
        .single_pixel([u32::MAX, u32::MAX / 2, 0, u32::MAX]);
    window.surface.attach(Some(&buffer), 0, 0);
    window.surface.damage_buffer(0, 0, 1, 1);
    window.surface.commit();
    harness.settle();

    let surface = harness.compositor().server().state.surface(id).unwrap();
    let current = harness
        .compositor()
        .textures
        .borrow()
        .get(&surface)
        .expect("the window has a texture");
    assert_eq!(current.texture.texture().size().width, 1);
    assert!(current.texture.opaque());
    assert_eq!(
        read(&device, &queue, current.texture.texture()),
        [0xff, 0x7f, 0, 0xff]
    );
}
