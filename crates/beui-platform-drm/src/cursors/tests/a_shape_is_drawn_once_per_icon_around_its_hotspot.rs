use be_host_dmabuf::testing::vulkan_device;

use super::*;

#[test]
fn a_shape_is_drawn_once_per_icon_around_its_hotspot() {
    let (device, queue) = vulkan_device();
    let gpu = Gpu::new(device, queue, FORMAT);
    let mut cursors = Cursors::new(2.0);
    assert!(
        cursors.get(&gpu, CursorIcon::None).is_none(),
        "a hidden cursor has no shape"
    );
    let text = cursors
        .get(&gpu, CursorIcon::Text)
        .expect("text has a shape");
    assert_eq!(
        (text.size, text.hotspot),
        (
            Vec2::splat(drawn_cursor::EXTENT),
            Vec2::splat(drawn_cursor::EXTENT / 2.0)
        ),
        "a shape keeps its size in points at any scale, with the hotspot in its middle"
    );
    let again = cursors
        .get(&gpu, CursorIcon::Text)
        .expect("text has a shape");
    assert!(Rc::ptr_eq(&text, &again), "each icon is drawn once");
    let touch = cursors
        .get(&gpu, CursorIcon::Touch)
        .expect("touch has a shape");
    assert!(!Rc::ptr_eq(&text, &touch));
}
