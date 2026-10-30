use super::*;
use block_plugin_api::{
    DockAlign, DockBar, DockBarEdge, DockBarPlacement, DockEdge, DockPlacedBar, Size,
};

fn own_bar(tab: u64, key: &str) -> DockBar {
    DockBar {
        tab: path(&[], tab),
        key: key.to_owned(),
        kind: format!("desktop/{key}"),
        owner: None,
        placement: DockBarPlacement::Beside(DockBarEdge {
            edge: DockEdge::Bottom,
            align: DockAlign::Center,
        }),
        size: Size {
            width: 160.0,
            height: 32.0,
        },
    }
}

#[test]
fn a_docks_own_bars_reach_the_root_and_come_back_to_each_level_as_its_own() {
    let mut docks = nested();
    let mut outer = outer();
    outer.bars.push(own_bar(7, "apps"));
    docks.heard("outer", vec![declare(OUTER, Some(outer))]);
    let mut inner = inner();
    inner.bars.push(own_bar(9, "tools"));
    docks.heard("inner", vec![declare(INNER, Some(inner))]);

    let delivered = docks
        .delivered
        .get(&(ROOT, OUTER_CHILD))
        .expect("the root heard its child");
    assert_eq!(
        delivered.bar(&path(&[], 7)).map(|bar| bar.owner.clone()),
        Some(None),
        "the child's own bar reaches the root as the child dock's, for the root to give the tab hosting it"
    );
    assert_eq!(
        delivered
            .bar(&path(&[INNER_CHILD], 9))
            .map(|bar| bar.owner.clone()),
        Some(Some(path(&[INNER_CHILD], 5))),
        "a deeper dock's own bar is lent to the first tab of its group"
    );

    let mut hosted = hosted_by_root();
    hosted.layout.bars = vec![
        DockPlacedBar {
            bar: path(&[], 7),
            owner: None,
            placement: DockBarPlacement::Beside(DockBarEdge {
                edge: DockEdge::Top,
                align: DockAlign::Center,
            }),
        },
        DockPlacedBar {
            bar: path(&[INNER_CHILD], 9),
            owner: Some(path(&[INNER_CHILD], 5)),
            placement: DockBarPlacement::Beside(DockBarEdge {
                edge: DockEdge::Left,
                align: DockAlign::Center,
            }),
        },
    ];
    host(&mut docks, DockHold::Hosted(hosted));
    assert_eq!(
        self::hosted(&mut docks, "outer", OUTER).layout.bars,
        [DockPlacedBar {
            bar: path(&[], 7),
            owner: None,
            placement: DockBarPlacement::Beside(DockBarEdge {
                edge: DockEdge::Top,
                align: DockAlign::Center,
            }),
        }]
    );
    assert_eq!(
        self::hosted(&mut docks, "inner", INNER).layout.bars,
        [DockPlacedBar {
            bar: path(&[], 9),
            owner: None,
            placement: DockBarPlacement::Beside(DockBarEdge {
                edge: DockEdge::Left,
                align: DockAlign::Center,
            }),
        }],
        "the deeper dock hears its bar back as its own"
    );
}
