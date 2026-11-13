use super::*;
use crate::{DockSpecBar, joined_key};

const TOP: DockBarPlacement = DockBarPlacement::Over(DockEdge::Top, DockAlign::Center);

fn bar(bar: u64, owner: u64) -> DockSpecBar {
    DockSpecBar {
        bar: TabId::new(bar),
        owner: Some(TabId::new(owner)),
        placement: TOP,
        hidden: false,
        pane: None,
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

    state.place_bar_on(TabId::new(21), None, TOP, 0);
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
    let restored = DockState::from_spec(&state.spec());
    assert_eq!(
        restored.pane_bars(TabId::new(2)),
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
    state.add_bar(TabId::new(30), Some(TabId::new(10)), TOP);
    state.add_bar(TabId::new(31), Some(TabId::new(10)), TOP);
    assert_eq!(
        state.pane_bars(TabId::new(10)),
        tabs(&[21, 20, 30, 31]),
        "the hosting tab's bars come before the hosted dock's, nearer the edge"
    );
    assert_eq!(state.bar_tab(TabId::new(20)), Some(TabId::new(10)));
    assert!(
        state.is_bar_shown(TabId::new(20), DockMode::Tiled),
        "the hosting tab's bar shows on the pane its dock shows in"
    );

    state.place_bar_on(TabId::new(31), None, TOP, 0);
    assert_eq!(
        state.pane_bars(TabId::new(10)),
        tabs(&[31, 21, 20, 30]),
        "a hosted bar dropped first goes first, before the outer layer's bars"
    );
    state.place_bar_on(
        TabId::new(20),
        None,
        DockBarPlacement::Beside(DockEdge::Bottom, DockAlign::Center),
        0,
    );
    state.place_bar_on(
        TabId::new(30),
        None,
        DockBarPlacement::Beside(DockEdge::Bottom, DockAlign::Center),
        0,
    );
    let bottom: Vec<TabId> = state
        .pane_bars(TabId::new(10))
        .into_iter()
        .filter(|held| {
            state.bar_placement(*held)
                == Some(DockBarPlacement::Beside(
                    DockEdge::Bottom,
                    DockAlign::Center,
                ))
        })
        .collect();
    assert_eq!(
        bottom,
        tabs(&[30, 20]),
        "on another edge too, a bar lands where it was dropped, whatever its layer"
    );
    state.add_bar(
        TabId::new(22),
        Some(TabId::new(2)),
        DockBarPlacement::Beside(DockEdge::Bottom, DockAlign::Center),
    );
    assert_eq!(
        state.pane_bars(TabId::new(10)),
        tabs(&[31, 21, 30, 20, 22]),
        "a bar new to the pane goes after the last of its own or an outer layer"
    );
    state.show(TabId::new(11));
    assert_eq!(
        state.bar_tab(TabId::new(20)),
        Some(TabId::new(11)),
        "the hosting tab's bars stay on the first pane of its dock, whichever tab it shows"
    );
    assert!(state.is_bar_shown(TabId::new(20), DockMode::Tiled));

    let mut fresh = DockState::default();
    fresh.reconcile(&spec(pane("main", &[1, 2])));
    fresh
        .host_group(
            TabId::new(2),
            &joined_key(TabId::new(2)),
            &pane("joined:2/inner", &[10]),
            Some(TabId::new(10)),
        )
        .expect("the hosted dock declares tabs");
    fresh.add_bar(TabId::new(30), Some(TabId::new(10)), TOP);
    fresh.add_bar(TabId::new(20), Some(TabId::new(2)), TOP);
    assert_eq!(
        fresh.pane_bars(TabId::new(10)),
        tabs(&[20, 30]),
        "an outer layer's bar new to a pane of only inner layers' bars goes before them"
    );
}
