use super::*;

#[test]
fn each_region_reads_its_own_viewport_size() {
    let _root = RootScope::new();
    let seen: Rc<RefCell<Vec<(EditorRegion, Vec2)>>> = Rc::default();
    let heard = Rc::clone(&seen);
    let mut surface = launched(
        &EditorHost::default(),
        Windowed::new(move |id| {
            let viewport = use_viewport_size();
            let heard = Rc::clone(&heard);
            create_effect(move || {
                let size = viewport.get();
                heard.borrow_mut().push((id.region, size));
            });
            view! {
                <styled::Frame />
            }
        }),
    );
    let frame = whole(40.0, 30.0);
    let mut preview = whole(20.0, 10.0);
    preview.region = EditorRegion::Preview;
    surface.frame(&[frame.clone(), preview.clone()]);
    surface.frame(&[frame, preview]);
    let last = |region: EditorRegion| {
        seen.borrow()
            .iter()
            .rev()
            .find(|(seen, _)| *seen == region)
            .map(|(_, size)| *size)
    };
    assert_eq!(
        last(EditorRegion::Frame),
        Some(vec2(40.0, 30.0)),
        "the frame reads its own size, not the document's"
    );
    assert_eq!(
        last(EditorRegion::Preview),
        Some(vec2(20.0, 10.0)),
        "a second region of the same document reads its own size"
    );
}
