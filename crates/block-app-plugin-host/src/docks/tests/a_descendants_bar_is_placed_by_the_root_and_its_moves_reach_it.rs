use super::*;
use block_plugin_api::{
    DockBar, DockBarMove, DockBarPlacement, DockDragStage, DockEdge, Pos2, Size,
};

#[test]
fn a_descendants_bar_is_placed_by_the_root_and_its_moves_reach_it() {
    let mut docks = nested();
    let mut inner = inner();
    inner.bars.push(DockBar {
        tab: path(&[], 9),
        key: "tools".to_owned(),
        kind: "canvas/tools".to_owned(),
        owner: path(&[], 5),
        placement: DockBarPlacement::Over(DockEdge::Bottom),
        size: Size {
            width: 160.0,
            height: 32.0,
        },
    });
    docks.heard("inner", vec![declare(INNER, Some(inner))]);
    host(&mut docks, DockHold::Hosted(hosted_by_root()));

    assert_eq!(
        docks.resolve(ROOT, &path(&[OUTER_CHILD, INNER_CHILD], 9)),
        Some((INNER_BLOCK, 9)),
        "the bar's body is no tab of the layout but is placed"
    );
    let delivered = docks
        .delivered
        .get(&(ROOT, OUTER_CHILD))
        .expect("the root heard its child");
    let bar = delivered
        .bar(&path(&[INNER_CHILD], 9))
        .expect("the bar is spliced through the child that declared it");
    assert_eq!(bar.owner, path(&[INNER_CHILD], 5));
    assert!(bar.key.ends_with("/tools"));

    updates(&mut docks, "shell");
    let moved = |bar| {
        DockRequest::MoveBar(DockBarMove {
            bar,
            at: Pos2::new(12.0, 8.0),
            stage: DockDragStage::Move,
        })
    };
    docks.heard(
        "inner",
        vec![DockMessage::Request(DockViewRequest {
            view: ViewRef {
                instance: INNER,
                view: ViewId::MAIN,
            },
            request: moved(path(&[], 9)),
        })],
    );
    assert_eq!(
        updates(&mut docks, "shell"),
        [(
            ROOT,
            DockUpdate::ChildRequest(DockChildRequest {
                child: OUTER_CHILD,
                owner: vec![INNER_CHILD],
                request: moved(path(&[INNER_CHILD], 9)),
            })
        )]
    );
}
