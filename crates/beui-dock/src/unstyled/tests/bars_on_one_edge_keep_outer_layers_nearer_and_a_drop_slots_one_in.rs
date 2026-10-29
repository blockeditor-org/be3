use super::*;
use crate::unstyled::state::{DockSpecBar, joined_key};

const TOP: DockBarPlacement = DockBarPlacement::Over(DockEdge::Top);

fn bar(bar: u64, owner: u64) -> DockSpecBar {
    DockSpecBar {
        bar: TabId::new(bar),
        owner: TabId::new(owner),
        placement: TOP,
    }
}

fn tabs(ids: &[u64]) -> Vec<TabId> {
    ids.iter().map(|id| TabId::new(*id)).collect()
}

#[test]
fn bars_on_one_edge_keep_outer_layers_nearer_and_a_drop_slots_one_in() {
    let mut state = DockState::default();
    let declared = DockSpec {
        bars: vec![bar(20, 2), bar(21, 2)],
        ..spec(pane("main", &[1, 2]))
    };
    state.reconcile(&declared);
    state.show(TabId::new(2));
    assert_eq!(
        state.pane_bars(TabId::new(2)),
        tabs(&[20, 21]),
        "one dock's bars stack in the order they are declared"
    );

    state.place_bar_at(TabId::new(21), TOP, 0);
    assert_eq!(
        state.pane_bars(TabId::new(2)),
        tabs(&[21, 20]),
        "a bar dropped before another takes its place"
    );
    state.reconcile(&declared);
    assert_eq!(
        state.pane_bars(TabId::new(2)),
        tabs(&[21, 20]),
        "the code declaring its bars again keeps the order the user chose"
    );
    let plain = DockState::from_plain(&state.plain());
    assert_eq!(
        plain.pane_bars(TabId::new(2)),
        tabs(&[21, 20]),
        "the order is saved with the layout"
    );

    state
        .host_group(
            TabId::new(2),
            &joined_key(TabId::new(2)),
            &pane("joined:2/inner", &[10, 11]),
            Some(TabId::new(10)),
        )
        .expect("the hosted dock declares tabs");
    state.add_bar(TabId::new(30), TabId::new(10), TOP);
    state.add_bar(TabId::new(31), TabId::new(10), TOP);
    assert_eq!(
        state.pane_bars(TabId::new(10)),
        tabs(&[21, 20, 30, 31]),
        "the hosting tab's bars come before the hosted dock's, nearer the edge"
    );
    assert_eq!(state.bar_tab(TabId::new(20)), Some(TabId::new(10)));
    assert!(
        state.is_bar_shown(TabId::new(20), false),
        "the hosting tab's bar shows on the pane its dock shows in"
    );

    state.place_bar_at(TabId::new(31), TOP, 0);
    assert_eq!(
        state.pane_bars(TabId::new(10)),
        tabs(&[21, 20, 31, 30]),
        "a hosted bar dropped first goes first among its own layer's bars"
    );
    state.place_bar_at(TabId::new(20), DockBarPlacement::Beside(DockEdge::Bottom), 0);
    state.place_bar_at(TabId::new(30), DockBarPlacement::Beside(DockEdge::Bottom), 0);
    let bottom: Vec<TabId> = state
        .pane_bars(TabId::new(10))
        .into_iter()
        .filter(|held| state.bar_placement(*held) == Some(DockBarPlacement::Beside(DockEdge::Bottom)))
        .collect();
    assert_eq!(
        bottom,
        tabs(&[20, 30]),
        "on another edge the outer layer's bar still comes first"
    );
    state.show(TabId::new(11));
    assert_eq!(
        state.bar_tab(TabId::new(20)),
        Some(TabId::new(11)),
        "the hosting tab's bars stay on the first pane of its dock, whichever tab it shows"
    );
    assert!(state.is_bar_shown(TabId::new(20), false));
}
