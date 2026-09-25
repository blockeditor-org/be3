use super::*;

#[test]
fn a_triangle_asks_to_be_painted_until_it_has_been() {
    let mut triangle = Triangle::new(EditorHost::default());
    let frame = triangle.update(&region(), None);
    assert!(frame.changed);
    assert_eq!(frame.repaint_after, None);
    assert_eq!(frame.painted, vec![region().rect]);
}
