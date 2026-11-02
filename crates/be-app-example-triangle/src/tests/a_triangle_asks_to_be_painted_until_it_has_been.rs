use super::*;

#[test]
fn a_triangle_asks_to_be_painted_until_it_has_been() {
    let host = EditorHost::default();
    let module = Module::default();
    host.join_module(&module);
    let mut triangle = Triangle::new(host.clone());
    triangle.open_view(View::MAIN);
    let mut handler = module
        .take_handler()
        .expect("the triangle handles the region it was asked to open");
    let frames = handler.frame(&[region()]);
    let [frame] = frames.as_slice() else {
        panic!("one region framed gets one frame");
    };
    assert!(frame.changed);
    assert_eq!(frame.repaint_after, None);
    assert_eq!(frame.painted, vec![region().rect]);
}
