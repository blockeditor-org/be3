use super::*;
use crate::unstyled::state::{DockMapping, DockSpecBar, joined_key};

const BOTTOM: DockBarPlacement = DockBarPlacement::Beside(DockEdge::Bottom, DockAlign::Center);

fn bar(bar: u64, owner: Option<u64>) -> DockSpecBar {
    DockSpecBar {
        bar: TabId::new(bar),
        owner: owner.map(TabId::new),
        placement: BOTTOM,
    }
}

#[test]
fn a_dock_bar_belongs_to_the_dock_and_shows_with_every_tab() {
    let mut state = DockState::default();
    let declared = DockSpec {
        bars: vec![bar(20, None), bar(21, Some(1)), bar(22, None)],
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
        state.bars_of(None),
        vec![TabId::new(20), TabId::new(22)],
        "the dock's own bars are the ones declared without a tab"
    );
    assert!(state.is_dock_bar(TabId::new(20)));
    assert!(!state.is_dock_bar(TabId::new(21)));
    assert_eq!(state.bar_owner(TabId::new(20)), None);
    assert_eq!(state.bar_tab(TabId::new(20)), None);
    assert_eq!(
        state.pane_bars(TabId::new(1)),
        vec![TabId::new(21)],
        "a pane holds only its tabs' bars"
    );
    assert_eq!(
        state.bars_with(TabId::new(20)),
        vec![TabId::new(20), TabId::new(22)]
    );

    for tab in [1, 2, 3] {
        state.show(TabId::new(tab));
        assert!(
            state.is_bar_shown(TabId::new(20), false),
            "the dock's bar shows while tab {tab} is shown"
        );
    }
    assert!(!state.is_bar_shown(TabId::new(21), false));

    state.place_bar_at(TabId::new(22), BOTTOM, 0);
    assert_eq!(
        state.bars_of(None),
        vec![TabId::new(22), TabId::new(20)],
        "a drop slots a dock bar in among the dock's bars"
    );
    state.set_bar_placement(
        TabId::new(20),
        DockBarPlacement::Over(DockEdge::Top, DockAlign::End),
    );
    state.reconcile(&declared);
    assert_eq!(
        state.bar_placement(TabId::new(20)),
        Some(DockBarPlacement::Over(DockEdge::Top, DockAlign::End)),
        "declared again, the bar keeps where it was moved"
    );
    let plain = DockState::from_plain(&state.plain());
    assert_eq!(plain.bars_of(None), state.bars_of(None));
    assert_eq!(
        plain.bar_placement(TabId::new(20)),
        state.bar_placement(TabId::new(20))
    );

    let mut root = DockState::default();
    let main = DockSpecNode::Pane(DockSpecPane {
        entries: vec![
            DockSpecEntry::Tab(TabId::new(1)),
            DockSpecEntry::Group {
                key: joined_key(TabId::new(2)),
                pinned: true,
                root: Box::new(pane("joined:2/stage", &[12, 13])),
            },
        ],
        ..pane_spec("main")
    });
    root.reconcile(&DockSpec {
        bars: vec![bar(30, None), bar(31, Some(2)), bar(32, Some(12))],
        ..spec(main)
    });
    let group = root
        .find_group(&joined_key(TabId::new(2)))
        .expect("the joined dock's group is open");
    let out = DockMapping {
        tab: &|tab| (tab.value() >= 10).then(|| TabId::new(tab.value() - 10)),
        key: &|key| key.strip_prefix("joined:2/").map(str::to_owned),
        group: &|_| None,
    };
    let mut joined = root.subtree(group, &out);
    joined.set_bar_placement(
        TabId::new(21),
        DockBarPlacement::Beside(DockEdge::Left, DockAlign::Center),
    );
    assert_eq!(
        joined.bars_of(None),
        vec![TabId::new(21)],
        "the joined dock sees the bar the root gave its host tab as its own"
    );
    assert_eq!(joined.bar_owner(TabId::new(22)), Some(TabId::new(2)));
    let back = DockMapping {
        tab: &|tab| Some(TabId::new(tab.value() + 10)),
        key: &|key| Some(format!("joined:2/{key}")),
        group: &|_| None,
    };
    root.graft(group, &joined, &back);
    assert_eq!(
        root.bar_owner(TabId::new(31)),
        Some(TabId::new(2)),
        "grafted back, the joined dock's own bar belongs to its host tab again"
    );
    assert_eq!(
        root.bar_placement(TabId::new(31)),
        Some(DockBarPlacement::Beside(DockEdge::Left, DockAlign::Center))
    );
    assert_eq!(root.bars_of(None), vec![TabId::new(30)]);
}
