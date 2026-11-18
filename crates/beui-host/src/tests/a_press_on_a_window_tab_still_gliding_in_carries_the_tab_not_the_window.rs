use super::*;
use crate::dock::TabId;
use crate::dock_host::unstyled::DockApi;

#[test]
fn a_press_on_a_window_tab_still_gliding_in_carries_the_tab_not_the_window() {
    let _root = crate::reactive::RootScope::new();
    let (document, dock) = dock_of(3);
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    let tab = harness.center(dock_tab(harness.document(), dock, "Tab 2"));
    drag_with(
        &mut harness,
        tab,
        pos2(WIDE_VIEWPORT.x / 2.0, WIDE_VIEWPORT.y / 2.0),
        Modifiers::ALT,
    );
    harness.settle();
    let state = harness.document().api::<DockApi>(dock).state();
    let window = state.windows()[0];
    let rect = state.window_rect(window).expect("the window has a rect");
    let origin = harness.rect(dock).min + rect.min.to_vec2();
    let floated = harness.rect(dock_tab(harness.document(), dock, "Tab 2"));
    let carried = harness.rect(dock_tab(harness.document(), dock, "Tab 3"));
    // Dropped past the end of the strip, the tab glides back along it to its place.
    harness.drag(
        pos2(carried.left() + 2.0, carried.center().y),
        pos2(floated.right() + 20.0, origin.y + 10.0),
    );
    harness.frame(Vec::new());
    let row = harness.rect(dock_tab(harness.document(), dock, "Tab 2"));
    let drawn = harness.rect(dock_tab(harness.document(), dock, "Tab 3"));
    assert!(
        drawn.left() > row.right() + 10.0,
        "the tab is still gliding, drawn past its place after Tab 2: {drawn:?} against {row:?}"
    );

    let placed = harness
        .document()
        .api::<DockApi>(dock)
        .state()
        .window_rect(window);
    harness.drag(
        pos2(drawn.center().x, drawn.center().y),
        pos2(WIDE_VIEWPORT.x - 20.0, WIDE_VIEWPORT.y - 40.0),
    );
    harness.frame(Vec::new());

    let state = harness.document().api::<DockApi>(dock).state();
    assert_eq!(
        state.window_rect(window),
        placed,
        "the window stays put: the press was on its tab, where the tab is drawn"
    );
    assert_eq!(
        state.surface_tabs(window),
        vec![TabId::new(2)],
        "and the tab was carried out of it"
    );
}
