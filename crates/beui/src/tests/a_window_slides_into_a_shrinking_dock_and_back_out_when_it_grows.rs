use super::*;
use crate::dock::unstyled::DockApi;

#[test]
fn a_window_slides_into_a_shrinking_dock_and_back_out_when_it_grows() {
    let _root = crate::reactive::RootScope::new();
    let (document, dock) = dock_of(2);
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    let window = floated_window(&mut harness, dock);
    let stored = harness
        .document()
        .api::<DockApi>(dock)
        .state()
        .window_rect(window)
        .expect("the window has a rect");
    let tab = |harness: &Harness| harness.rect(dock_tab(harness.document(), dock, "Tab 2"));
    let inset = tab(&harness).min - (harness.rect(dock).min + stored.min.to_vec2());

    harness.resize(vec2(stored.min.x + 60.0, stored.min.y + 20.0));
    harness.frame(Vec::new());
    harness.frame(Vec::new());

    let shrunk = harness.rect(dock);
    let drawn = tab(&harness).min - inset;
    assert!(
        drawn.x < shrunk.max.x - 60.0 && drawn.y < shrunk.max.y - 20.0,
        "the window is pulled back so its grip and bar stay inside the smaller dock: {drawn:?} in {shrunk:?}"
    );
    assert_eq!(
        harness
            .document()
            .api::<DockApi>(dock)
            .state()
            .window_rect(window),
        Some(stored),
        "shrinking the dock leaves where the window belongs alone"
    );

    harness.resize(WIDE_VIEWPORT);
    harness.frame(Vec::new());
    harness.frame(Vec::new());

    assert_eq!(
        tab(&harness).min - inset,
        harness.rect(dock).min + stored.min.to_vec2(),
        "growing the dock again puts the window back where it was"
    );
}
