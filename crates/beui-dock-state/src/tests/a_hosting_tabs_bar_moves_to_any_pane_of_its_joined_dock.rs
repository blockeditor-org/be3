use super::*;
use crate::joined_key;

const TOP: DockBarPlacement = DockBarPlacement::Over(DockEdge::Top, DockAlign::Center);

fn tabs(ids: &[u64]) -> Vec<TabId> {
    ids.iter().map(|id| TabId::new(*id)).collect()
}

#[test]
fn a_hosting_tabs_bar_moves_to_any_pane_of_its_joined_dock() {
    let mut state = DockState::default();
    state.reconcile(&spec(pane("main", &[1, 2])));
    state.show(TabId::new(2));
    state.add_bar(TabId::new(20), Some(TabId::new(2)), TOP);
    state
        .host_group(
            TabId::new(2),
            &joined_key(TabId::new(2)),
            &split(
                "joined:2/split",
                Direction::Horizontal,
                0.5,
                pane("joined:2/left", &[10]),
                pane("joined:2/right", &[11]),
            ),
            Some(TabId::new(10)),
        )
        .expect("the hosted dock declares tabs");
    state.add_bar(TabId::new(30), Some(TabId::new(11)), TOP);
    assert_eq!(
        state.bar_tab(TabId::new(20)),
        Some(TabId::new(10)),
        "the hosting tab's bar starts on its joined dock's first pane"
    );
    assert_eq!(
        state.bar_panes(TabId::new(20)),
        tabs(&[10, 11]),
        "and may go to any of its panes"
    );
    assert_eq!(
        state.bar_panes(TabId::new(30)),
        tabs(&[11]),
        "while a joined tab's own bar stays on its own pane"
    );

    state.set_bar_pane(TabId::new(20), TabId::new(11));
    state.place_bar_at(TabId::new(20), TOP, 0);
    assert_eq!(state.bar_tab(TabId::new(20)), Some(TabId::new(11)));
    assert_eq!(
        state.pane_bars(TabId::new(11)),
        tabs(&[20, 30]),
        "dropped on the second pane, it shows there, the outer layer nearer the edge"
    );
    assert_eq!(state.pane_bars(TabId::new(10)), Vec::<TabId>::new());
    let restored = DockState::from_spec(&state.spec());
    assert_eq!(
        restored.bar_tab(TabId::new(20)),
        Some(TabId::new(11)),
        "the pane is saved with the layout"
    );

    state.set_bar_pane(TabId::new(20), TabId::new(1));
    assert_eq!(
        state.bar_tab(TabId::new(20)),
        Some(TabId::new(10)),
        "a tab outside its joined dock is no pane for it: it goes back to the first"
    );
}
