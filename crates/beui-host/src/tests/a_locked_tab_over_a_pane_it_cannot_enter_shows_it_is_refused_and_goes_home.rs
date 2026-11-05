use super::*;

#[test]
fn a_locked_tab_over_a_pane_it_cannot_enter_shows_it_is_refused_and_goes_home() {
    let _root = crate::reactive::RootScope::new();
    let (document, nested) = nested_dock();
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    let state = nested.outer.state().get_untracked();
    let group = state.find_group("joined:2").expect("the inner dock joined");
    let tabs = state.group_tabs(group);
    let home = harness.rect(dock_tab(harness.document(), nested.node, "Inner 10"));
    let into_side = pos2(WIDE_VIEWPORT.x * 0.15, WIDE_VIEWPORT.y / 2.0);

    harness.down(home.center(), PointerButton::Primary);
    for step in 1..=8 {
        let at = home.center() + (into_side - home.center()) * (step as f32 / 8.0);
        harness.hover(at);
    }
    harness.frame(Vec::new());
    assert!(
        harness.document().find_test_id("dock.refused").is_some(),
        "held over a pane its locked group keeps it out of, the tab shows it is refused"
    );
    assert!(
        harness.document().find_test_id("dock.drop").is_none(),
        "and no drop is offered there"
    );

    harness.up(into_side, PointerButton::Primary);
    harness.frames_to_settle();
    assert!(
        harness.document().find_test_id("dock.refused").is_none(),
        "let go, the refusal goes away"
    );
    assert_eq!(
        nested.outer.state().get_untracked().group_tabs(group),
        tabs,
        "nothing takes the tab"
    );
    assert_eq!(
        harness.rect(dock_tab(harness.document(), nested.node, "Inner 10")),
        home,
        "and it glides back to its place in its bar"
    );
}
