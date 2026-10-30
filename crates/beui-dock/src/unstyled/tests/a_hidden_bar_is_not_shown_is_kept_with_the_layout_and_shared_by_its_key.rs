use super::*;
use crate::unstyled::state::DockSpecBar;

const BOTTOM: DockBarPlacement = DockBarPlacement::Beside(DockEdge::Bottom, DockAlign::Center);

fn bar(bar: u64, owner: Option<u64>) -> DockSpecBar {
    DockSpecBar {
        bar: TabId::new(bar),
        owner: owner.map(TabId::new),
        placement: BOTTOM,
    }
}

#[test]
fn a_hidden_bar_is_not_shown_is_kept_with_the_layout_and_shared_by_its_key() {
    let mut state = DockState::default();
    let declared = DockSpec {
        bars: vec![bar(20, None), bar(21, Some(1)), bar(22, Some(2))],
        ..spec(pane("main", &[1, 2]))
    };
    state.reconcile(&declared);
    assert_eq!(state.dock_band_edges(), vec![DockEdge::Bottom]);

    state.set_bar_hidden(TabId::new(20), true);
    assert!(state.is_bar_hidden(TabId::new(20)));
    assert!(
        !state.is_bar_shown(TabId::new(20), false),
        "a hidden dock bar shows with no tab"
    );
    assert!(
        state.dock_band_edges().is_empty(),
        "a hidden bar takes no band"
    );
    assert_eq!(
        state.bar_placement(TabId::new(20)),
        Some(BOTTOM),
        "it keeps its placement while hidden"
    );

    let kept = DockState::from_plain(&state.plain());
    assert!(
        kept.is_bar_hidden(TabId::new(20)),
        "the hidden flag is saved and restored with the layout"
    );
    state.reconcile(&declared);
    assert!(
        state.is_bar_hidden(TabId::new(20)),
        "declared again, the bar stays hidden"
    );

    state.show(TabId::new(1));
    state.set_bar_hidden(TabId::new(21), true);
    let key = |bar: TabId| match bar.value() {
        21 | 22 => "tools".to_owned(),
        other => format!("#{other}"),
    };
    state.share_bar_placement(TabId::new(21), &key);
    assert!(
        state.is_bar_hidden(TabId::new(22)),
        "hiding a bar hides every bar of its key"
    );
    assert!(!state.is_bar_shown(TabId::new(21), false));

    state.set_bar_hidden(TabId::new(20), false);
    assert!(state.is_bar_shown(TabId::new(20), false));
    assert_eq!(state.dock_band_edges(), vec![DockEdge::Bottom]);
}
