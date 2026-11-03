use super::*;

#[test]
fn a_tab_whose_pane_is_gone_joins_the_nearest_open_tab() {
    let mut state = DockState::default();
    let layout = |right: &[u64]| {
        spec(split(
            "root",
            Direction::Horizontal,
            0.5,
            pane("left", &[1, 5]),
            pane("right", right),
        ))
    };
    state.reconcile(&layout(&[2]));
    state.reconcile(&layout(&[]));
    assert_eq!(
        state.leaves(state.main()).len(),
        1,
        "a pane without an empty view goes when its last tab does"
    );

    state.reconcile(&layout(&[3]));
    let left = state.find(TabId::new(1)).expect("tab 1 is open").leaf;
    assert_eq!(
        state.entries(left),
        vec![
            Entry::Tab(TabId::new(1)),
            Entry::Tab(TabId::new(5)),
            Entry::Tab(TabId::new(3))
        ],
        "with its pane gone, the tab joins the nearest declared tab still open"
    );
}
