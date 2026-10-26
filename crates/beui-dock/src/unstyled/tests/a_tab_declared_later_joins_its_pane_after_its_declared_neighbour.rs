use super::*;

#[test]
fn a_tab_declared_later_joins_its_pane_after_its_declared_neighbour() {
    let mut state = DockState::default();
    let layout = |tabs: &[u64]| {
        spec(split(
            "root",
            Direction::Horizontal,
            0.5,
            pane("left", &[1]),
            pane("right", tabs),
        ))
    };
    state.reconcile(&layout(&[2, 4]));
    state.reconcile(&layout(&[2, 3, 4]));

    let right = state.find(TabId::new(2)).expect("tab 2 is open").leaf;
    assert_eq!(
        state.entries(right),
        vec![
            Entry::Tab(TabId::new(2)),
            Entry::Tab(TabId::new(3)),
            Entry::Tab(TabId::new(4))
        ],
        "the new tab lands right after the tab declared before it"
    );
    assert_eq!(
        state.focused_tab(),
        Some(TabId::new(3)),
        "a tab that appears is shown"
    );
}
