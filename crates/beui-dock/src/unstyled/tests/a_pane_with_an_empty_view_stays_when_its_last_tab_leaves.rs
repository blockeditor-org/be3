use super::*;

#[test]
fn a_pane_with_an_empty_view_stays_when_its_last_tab_leaves() {
    let mut state = DockState::default();
    let layout = |right: &[u64]| {
        spec(split(
            "root",
            Direction::Horizontal,
            0.5,
            pane("left", &[1]),
            kept("right", right),
        ))
    };
    state.reconcile(&layout(&[2]));
    let left = state.find(TabId::new(1)).expect("tab 1 is open").leaf;
    state.drop_tab(TabId::new(2), DockDrop::Pane { leaf: left });
    state.reconcile(&layout(&[2]));

    let leaves = state.leaves(state.main());
    assert_eq!(leaves.len(), 2, "the pane that keeps itself is not closed");
    let right = leaves
        .into_iter()
        .find(|leaf| *leaf != left)
        .expect("the kept pane is still there");
    assert!(
        state.entries(right).is_empty(),
        "the kept pane shows its empty view"
    );

    state.reconcile(&layout(&[2, 3]));
    assert_eq!(
        state.find(TabId::new(3)).map(|position| position.leaf),
        Some(right),
        "a tab declared in the kept pane lands in it"
    );
}
