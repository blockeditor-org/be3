use super::*;

#[test]
fn a_region_lays_out_over_its_whole_rect_not_just_what_shows() {
    let _root = RootScope::new();
    let laid = NodeRef::new();
    let document = Rc::default();
    let mut surface = launched(
        &EditorHost::default(),
        Windowed::new(move |_| {
            view! {
                <styled::Frame @node_ref=&laid />
            }
        })
        .sharing(&document),
    );
    let scrolled = region(vec2(100.0, 50.0), vec2(10.0, 20.0), [80, 30]);
    one(&mut surface, &scrolled);
    let rect = document
        .borrow()
        .as_ref()
        .and_then(|document: &Document| document.node_rect(laid.get()));
    assert_eq!(
        rect,
        Some(Rect::from_min_size(Pos2::ZERO, vec2(100.0, 50.0))),
        "a region scrolled partly out of view is laid out over all of its window, with the surface showing the part still in view"
    );
}
