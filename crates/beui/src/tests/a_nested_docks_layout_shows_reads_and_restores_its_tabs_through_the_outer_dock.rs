use super::*;
use crate::dock::unstyled::{DockDrop, DockingSnapshot, Side};

#[test]
fn a_nested_docks_layout_shows_reads_and_restores_its_tabs_through_the_outer_dock() {
    let _root = crate::reactive::RootScope::new();
    let (document, nested) = nested_dock();
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    let inner = nested.inner;
    assert_eq!(
        inner.focused(),
        Some(10),
        "the focus moved from the hosting tab into the inner dock"
    );

    inner.show(&11);
    harness.frame(Vec::new());
    assert_eq!(inner.focused(), Some(11), "showing a tab goes through the outer dock");
    assert_eq!(nested.outer.focused(), None, "whose own keys do not name it");
    let shown = nested.outer.state().get_untracked().focused_tab();
    assert!(shown.is_some());

    let snapshot = inner.snapshot();
    let mut keys: Vec<u64> = snapshot.keys().copied().collect();
    keys.sort();
    assert_eq!(keys, vec![10, 11], "the snapshot is the inner dock's subtree");

    let mut state = snapshot.state().clone();
    let eleven = inner.tab_of(&11).expect("tab 11 is keyed");
    let ten = inner.tab_of(&10).expect("tab 10 is keyed");
    let leaf = state.find(eleven).expect("tab 11 is in the snapshot").leaf;
    state.drop_tab(eleven, DockDrop::Split { leaf, side: Side::Right });
    inner.restore(DockingSnapshot::new(state, [(10, ten), (11, eleven)]));
    harness.frame(Vec::new());

    let outer = nested.outer.state().get_untracked();
    let group = outer.find_group("joined:2").expect("the group is still there");
    assert_eq!(
        outer.tree_leaves(crate::dock::unstyled::Tree::Group(group)).len(),
        2,
        "restoring through the inner layout splits the group in the outer dock"
    );
    assert_eq!(
        outer.find_group("joined:2").map(|group| outer.group_tabs(group).len()),
        Some(2)
    );
    assert_eq!(
        inner.state().get_untracked().leaves(inner.state().get_untracked().main()).len(),
        2,
        "and the inner layout reads the split back"
    );
}
