use super::*;

#[test]
fn every_view_is_a_window_painted_in_the_frame_that_first_asks_for_it() {
    let _root = RootScope::new();
    let mut surface = launched(
        &EditorHost::default(),
        Windowed::new(|_| {
            view! {
                <styled::Frame fill=styled::ColorRole::Accent />
            }
        }),
    );
    let frame = whole(40.0, 30.0);
    let mut preview = whole(20.0, 10.0);
    preview.view = block_editor_plugin::View::PREVIEW;

    let first = surface.frame(std::slice::from_ref(&frame));
    assert!(
        first[0].changed,
        "a region is drawn in the frame that first shows it"
    );
    let framed = surface
        .regions()
        .window(frame.id())
        .expect("the app opened a window for the region in that same frame");
    assert!(surface.painting(frame.id()).is_some());

    let both = surface.frame(&[frame.clone(), preview.clone()]);
    assert!(
        both[1].changed,
        "a region shown beside another is drawn in the frame that first shows it"
    );
    let previewed = surface
        .regions()
        .window(preview.id())
        .expect("the second region is a window of the same app");
    assert_ne!(framed, previewed);
    assert_eq!(surface.regions().window(frame.id()), Some(framed));
    let painting = surface
        .painting(preview.id())
        .expect("the second region paints at once");
    assert_eq!(painting.frame.size, [20, 10]);

    surface.frame(std::slice::from_ref(&frame));
    assert_eq!(
        surface.regions().window(preview.id()),
        None,
        "a region the host stops showing closes its window"
    );
    assert_eq!(surface.regions().window(frame.id()), Some(framed));
}
