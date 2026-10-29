use super::*;
use crate::unstyled::state::{DockPlain, DockPlainEntry, DockPlainHome, DockPlainNode, DockPlainPlace};

fn plain_pane(key: &str, area: bool, tabs: &[u64]) -> DockPlainNode {
    DockPlainNode::Pane {
        key: key.to_owned(),
        active: 0,
        vertical: false,
        icons_only: false,
        sidebar: SIDEBAR_WIDTH,
        keep: false,
        area: area.then(|| key.to_owned()),
        bare: false,
        entries: tabs
            .iter()
            .map(|tab| DockPlainEntry::Tab(TabId::new(*tab)))
            .collect(),
    }
}

fn deck(stage: Vec<DockSpecEntry>) -> DockSpec {
    let mut pane = pane_spec("joined:2/stage");
    pane.entries = stage;
    let mut editors = pane_spec("editors");
    editors.locked = true;
    editors.keep = true;
    editors.entries = vec![DockSpecEntry::Group {
        key: "joined:2".to_owned(),
        pinned: true,
        root: Box::new(split(
            "joined:2/deck",
            Direction::Horizontal,
            0.2,
            locked("joined:2/filmstrip", &[10]),
            DockSpecNode::Pane(pane),
        )),
    }];
    spec(split(
        "workspace",
        Direction::Horizontal,
        0.25,
        locked("files", &[1]),
        DockSpecNode::Pane(editors),
    ))
}

#[test]
fn a_dock_joining_through_an_emptied_pane_of_a_group_lands_in_that_pane() {
    let editors = DockPlainNode::Pane {
        key: "editors".to_owned(),
        active: 0,
        vertical: false,
        icons_only: false,
        sidebar: SIDEBAR_WIDTH,
        keep: true,
        area: Some("editors".to_owned()),
        bare: false,
        entries: vec![DockPlainEntry::Group {
            key: "joined:2".to_owned(),
            pinned: true,
            root: Box::new(DockPlainNode::Split {
                key: "joined:2/deck".to_owned(),
                direction: Direction::Horizontal,
                fraction: 0.2,
                first: Box::new(plain_pane("joined:2/filmstrip", true, &[10])),
                second: Box::new(plain_pane("joined:2/stage", false, &[11])),
            }),
        }],
    };
    let mut state = DockState::from_plain(&DockPlain {
        main: Some(DockPlainNode::Split {
            key: "workspace".to_owned(),
            direction: Direction::Horizontal,
            fraction: 0.25,
            first: Box::new(plain_pane("files", true, &[1])),
            second: Box::new(editors),
        }),
        windows: Vec::new(),
        focus: Some(TabId::new(11)),
        homes: vec![
            DockPlainHome {
                tab: TabId::new(1),
                place: DockPlainPlace::Area("files".to_owned()),
            },
            DockPlainHome {
                tab: TabId::new(11),
                place: DockPlainPlace::Group("joined:2".to_owned()),
            },
            DockPlainHome {
                tab: TabId::new(10),
                place: DockPlainPlace::Area("joined:2/filmstrip".to_owned()),
            },
        ],
        bars: vec![crate::unstyled::state::DockPlainBar {
            bar: TabId::new(20),
            owner: TabId::new(11),
            placement: DockBarPlacement::Beside(DockEdge::Top),
        }],
        outside: Vec::new(),
        unlocked: Vec::new(),
    });
    state.mark_seeded();
    let mut first = deck(vec![DockSpecEntry::Tab(TabId::new(11))]);
    first.bars = vec![DockSpecBar {
        bar: TabId::new(20),
        owner: TabId::new(11),
        placement: DockBarPlacement::Beside(DockEdge::Top),
    }];
    state.reconcile(&first);

    let mut inner = pane_spec("joined:2/stage/slide/canvas");
    inner.bare = true;
    inner.entries = vec![DockSpecEntry::Tab(TabId::new(12))];
    let mut joining = deck(vec![DockSpecEntry::Group {
        key: "joined:2/stage/slide".to_owned(),
        pinned: true,
        root: Box::new(DockSpecNode::Pane(inner)),
    }]);
    joining.bars = vec![
        DockSpecBar {
            bar: TabId::new(20),
            owner: TabId::new(12),
            placement: DockBarPlacement::Beside(DockEdge::Top),
        },
        DockSpecBar {
            bar: TabId::new(21),
            owner: TabId::new(12),
            placement: DockBarPlacement::Over(DockEdge::Bottom),
        },
    ];
    state.reconcile(&joining);
    let (leaf, _) = state
        .locate(Entry::Group(
            state.find_group("joined:2/stage/slide").expect("the joined dock is placed"),
        ))
        .expect("its group is placed");
    assert_eq!(
        state.pane_key(leaf),
        Some("joined:2/stage"),
        "the joined dock lands in the pane that held the tab it took the place of"
    );
}
