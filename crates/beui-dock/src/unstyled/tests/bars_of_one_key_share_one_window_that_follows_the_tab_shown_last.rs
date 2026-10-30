use super::*;
use crate::unstyled::state::DockSpecBar;
use beui_view::types::Pos2;

fn key(bar: TabId) -> String {
    match bar.value() {
        20 | 21 => "tools".to_owned(),
        _ => format!("#{}", bar.value()),
    }
}

#[test]
fn bars_of_one_key_share_one_window_that_follows_the_tab_shown_last() {
    let mut state = DockState::default();
    let bar = |bar: u64, owner: u64| DockSpecBar {
        bar: TabId::new(bar),
        owner: Some(TabId::new(owner)),
        placement: DockBarPlacement::Over(DockEdge::Bottom, DockAlign::Center),
    };
    let declared = DockSpec {
        bars: vec![bar(20, 1), bar(21, 2), bar(22, 1)],
        ..spec(split(
            "root",
            Direction::Horizontal,
            0.5,
            pane("left", &[1, 2]),
            pane("right", &[3]),
        ))
    };
    state.reconcile(&declared);
    let window = DockBarPlacement::Window {
        edge: DockEdge::Bottom,
        origin: Pos2::new(40.0, 30.0),
    };

    state.set_bar_placement(TabId::new(20), window);
    state.share_bar_placement(TabId::new(20), &key);
    assert_eq!(
        state.bar_placement(TabId::new(21)),
        Some(window),
        "popping out one bar pops out every bar of its key"
    );
    assert_eq!(
        state.bar_placement(TabId::new(22)),
        Some(DockBarPlacement::Over(DockEdge::Bottom, DockAlign::Center)),
        "a bar of another key stays where it was"
    );
    assert_eq!(
        state.bar_windows(&key),
        vec![("tools".to_owned(), vec![TabId::new(20), TabId::new(21)])],
        "the bars of one key share one window"
    );

    let shared = [TabId::new(20), TabId::new(21)];
    state.show(TabId::new(2));
    assert_eq!(
        state.followed_bar(&shared),
        Some(TabId::new(21)),
        "the window shows the bar of the tab shown last"
    );
    state.show(TabId::new(1));
    assert_eq!(state.followed_bar(&shared), Some(TabId::new(20)));
    state.show(TabId::new(3));
    assert_eq!(
        state.followed_bar(&shared),
        Some(TabId::new(20)),
        "focusing a tab with no such bar keeps following the one before"
    );

    let left = leaf_of(&state, 1);
    state.drop_tab(TabId::new(3), DockDrop::Pane { leaf: left });
    state.show(TabId::new(3));
    assert_eq!(
        state.followed_bar(&shared),
        None,
        "while no tab with a bar of the key is shown, there is nothing to show"
    );

    let mut later = DockState::default();
    later.reconcile(&DockSpec {
        bars: vec![bar(20, 1)],
        ..declared.clone()
    });
    later.set_bar_placement(TabId::new(20), window);
    let mut arrived = later.clone();
    arrived.reconcile(&DockSpec {
        bars: vec![bar(20, 1), bar(21, 2)],
        ..declared.clone()
    });
    arrived.adopt_bar_placements(&later, &key);
    assert_eq!(
        arrived.bar_placement(TabId::new(21)),
        Some(window),
        "a bar arriving later takes the placement of the bars of its key"
    );

    state.set_bar_placement(TabId::new(21), DockBarPlacement::Beside(DockEdge::Left));
    state.share_bar_placement(TabId::new(21), &key);
    assert_eq!(
        state.bar_placement(TabId::new(20)),
        Some(DockBarPlacement::Beside(DockEdge::Left)),
        "docking one back docks them all"
    );
    assert!(state.bar_windows(&key).is_empty());
}
