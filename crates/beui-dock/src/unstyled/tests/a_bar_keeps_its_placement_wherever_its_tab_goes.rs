use super::*;
use crate::unstyled::state::DockSpecBar;

#[test]
fn a_bar_keeps_its_placement_wherever_its_tab_goes() {
    let mut state = DockState::default();
    let bars = vec![
        DockSpecBar {
            bar: TabId::new(20),
            owner: Some(TabId::new(1)),
            placement: DockBarPlacement::Over(DockEdge::Left, DockAlign::Center),
            hidden: false,
        },
        DockSpecBar {
            bar: TabId::new(21),
            owner: Some(TabId::new(1)),
            placement: DockBarPlacement::Beside(DockEdge::Bottom, DockAlign::Center),
            hidden: false,
        },
    ];
    let declared = DockSpec {
        bars: bars.clone(),
        ..spec(split(
            "root",
            Direction::Horizontal,
            0.5,
            pane("left", &[1, 2]),
            pane("right", &[3]),
        ))
    };
    state.reconcile(&declared);
    assert_eq!(
        state.bars_of(Some(TabId::new(1))),
        vec![TabId::new(20), TabId::new(21)]
    );
    assert_eq!(
        state.bar_placement(TabId::new(20)),
        Some(DockBarPlacement::Over(DockEdge::Left, DockAlign::Center)),
        "a bar starts where it is declared"
    );

    state.set_bar_placement(
        TabId::new(20),
        DockBarPlacement::Beside(DockEdge::Right, DockAlign::Center),
    );
    state.show(TabId::new(1));
    assert!(state.is_bar_shown(TabId::new(20), false));
    state.show(TabId::new(2));
    assert!(
        !state.is_bar_shown(TabId::new(20), false),
        "a bar shows only while its tab does"
    );

    let right = leaf_of(&state, 3);
    state.drop_tab(TabId::new(1), DockDrop::Pane { leaf: right });
    state.reconcile(&declared);
    assert_eq!(
        state.bar_placement(TabId::new(20)),
        Some(DockBarPlacement::Beside(DockEdge::Right, DockAlign::Center)),
        "the bar keeps the placement it was moved to when its tab moves and the code declares it again"
    );
    assert!(state.is_bar_shown(TabId::new(20), false));

    let restored = DockState::from_spec(&state.spec());
    assert_eq!(
        restored.bars_of(Some(TabId::new(1))),
        state.bars_of(Some(TabId::new(1)))
    );
    assert_eq!(
        restored.bar_placement(TabId::new(21)),
        Some(DockBarPlacement::Beside(
            DockEdge::Bottom,
            DockAlign::Center
        ))
    );

    state.reconcile(&DockSpec {
        bars: vec![bars[1].clone()],
        ..declared.clone()
    });
    assert_eq!(
        state.bars(),
        vec![TabId::new(21)],
        "a bar the code stops declaring leaves"
    );
}
