use super::*;

#[test]
fn a_blocking_window_of_its_own_is_left_out_of_the_cover_its_window_reports() {
    let _root = crate::reactive::RootScope::new();
    let window = WindowId::next();
    let size = vec2(200.0, 120.0);
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <Frame width=600.0 height=400.0 />
                <Window
                    id={window}
                    size={size}
                    anchor={Some(WindowAnchor::Centre)}
                    blocks=true
                    scrim={SCRIM}
                >|_: WindowHandle|
                    <Frame />
                </Window>
            </List>
        }
    });
    let mut harness = Harness::hosting_windows(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    let at = harness
        .take_window_requests()
        .iter()
        .find_map(|request| match request {
            WindowRequest::Open {
                window: opened,
                placement: WindowPlacement::Over { at, .. },
                ..
            } if *opened == window => Some(*at),
            _ => None,
        })
        .expect("it opens over the main window");
    let shown = Rect::from_min_size(at, size);
    let covers = harness.document().overlay_rects(WindowId::MAIN);
    assert!(
        covers.iter().all(|cover| !cover.intersects(shown)),
        "a host lifting the main window's cover over other windows leaves the dialog's own \
         window to take its presses: {covers:?} {shown:?}"
    );
    let covered: f32 = covers.iter().map(|cover| cover.width() * cover.height()).sum();
    assert_eq!(
        covered,
        WIDE_VIEWPORT.x * WIDE_VIEWPORT.y - size.x * size.y,
        "and covers all the rest of its window"
    );
}
