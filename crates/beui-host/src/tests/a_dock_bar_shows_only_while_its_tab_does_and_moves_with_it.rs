use super::*;
use crate::dock::unstyled::{DockBarPlacement, DockDrop, DockEdge, TabId};

#[test]
fn a_dock_bar_shows_only_while_its_tab_does_and_moves_with_it() {
    let _root = crate::reactive::RootScope::new();
    let (document, dock) = bar_dock(DockBarPlacement::Beside(DockEdge::Top), false);
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    dock.shown.set(vec![1, 3]);
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    dock.layout.show(&1);
    harness.frame(Vec::new());
    assert!(harness.document().find_test_id("bar.9").is_some());

    dock.layout.show(&3);
    harness.frame(Vec::new());
    assert!(
        harness.document().find_test_id("bar.9").is_none(),
        "another tab shown in the pane hides the bar"
    );

    dock.layout.show(&1);
    harness.frame(Vec::new());
    let side = harness.rect(harness.find("content.2"));
    let state = dock.layout.state().get_untracked();
    let leaf = state
        .find(TabId::new(2))
        .expect("the side tab is open")
        .leaf;
    let mut moved = state.clone();
    moved.drop_tab(TabId::new(1), DockDrop::Pane { leaf });
    let keys = [
        (1u64, TabId::new(1)),
        (2, TabId::new(2)),
        (3, TabId::new(3)),
        (9, TabId::new(9)),
    ];
    dock.layout
        .restore(crate::dock::unstyled::DockingSnapshot::new(moved, keys));
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    let bar = harness.rect(harness.find("bar.9"));
    assert!(
        bar.left() >= side.left() - 1.0,
        "the bar went with its tab into the other pane"
    );
    assert_eq!(
        dock.layout.bar_placement(&9),
        Some(DockBarPlacement::Beside(DockEdge::Top))
    );
}
