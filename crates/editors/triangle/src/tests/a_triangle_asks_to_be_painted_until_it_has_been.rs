use super::*;

#[test]
fn a_triangle_asks_to_be_painted_until_it_has_been() {
    let host = EditorHost::default();
    let mut triangle = Triangle::new(host.clone());
    triangle.open_region(EditorRegion::Frame);
    let mut handlers = host.take_region_handlers();
    let (opened, mut handler) = handlers
        .pop()
        .expect("the triangle handles the region it was asked to open");
    assert_eq!(opened, EditorRegion::Frame);
    let frame = handler.update(&region());
    assert!(frame.changed);
    assert_eq!(frame.repaint_after, None);
    assert_eq!(frame.painted, vec![region().rect]);
}
