use super::*;
use block_plugin_api::{
    DockAlign, DockBar, DockBarMove, DockBarEdge, DockBarPlacement, DockCover, DockDragStage,
    DockEdge, Pos2, Rect, Size,
};

const FRAMER: EditorInstanceId = EditorInstanceId(4);
const FRAME_CHILD: ChildId = ChildId(30);
const FRAMER_BLOCK: Uuid = Uuid::from_u128(0xc);

fn bar(tab: u64, owner: u64, placement: DockBarPlacement) -> DockBar {
    DockBar {
        tab: path(&[], tab),
        key: "tools".to_owned(),
        kind: "canvas/tools".to_owned(),
        owner: Some(path(&[], owner)),
        placement,
        size: Size {
            width: 160.0,
            height: 32.0,
        },
    }
}

fn root_hosts(docks: &mut Docks, bars: Vec<DockPlacedBar>, covers: Vec<DockCover>) {
    let delivered = docks.delivered.get(&(ROOT, OUTER_CHILD)).unwrap().clone();
    host(
        docks,
        DockHold::Hosted(DockHosted {
            layout: DockLayout {
                tree: delivered.tree.clone(),
                homes: Vec::new(),
                bars,
                outside: Vec::new(),
                unlocked: Vec::new(),
            },
            tabs: delivered.tabs.iter().map(|tab| tab.tab.clone()).collect(),
            focused: None,
            shown: None,
            visible: vec![path(&[], 1)],
            fullscreen: None,
            stacked: false,
            covers,
        }),
    );
}

fn delivered_bars(docks: &Docks) -> Vec<(DockTabPath, Option<DockTabPath>, DockBarPlacement)> {
    docks
        .delivered
        .get(&(ROOT, OUTER_CHILD))
        .unwrap()
        .bars
        .iter()
        .map(|bar| (bar.tab.clone(), bar.owner.clone(), bar.placement.clone()))
        .collect()
}

#[test]
fn a_frame_childs_bars_take_the_place_of_its_placers_and_give_it_back() {
    let mut docks = Docks::default();
    docks.register(ROOT, "shell", Some(Uuid::from_u128(1)));
    docks.register(OUTER, "outer", Some(OUTER_BLOCK));
    docks.register(FRAMER, "framer", Some(FRAMER_BLOCK));
    docks.placed(
        ROOT,
        &[DockedChild {
            child: OUTER_CHILD,
            tab: 100,
            block: OUTER_BLOCK,
        }],
        None,
    );
    let mut outer = outer();
    outer.bars.push(bar(
        3,
        1,
        DockBarPlacement::Over(DockBarEdge {
            edge: DockEdge::Bottom,
            align: DockAlign::Center,
        }),
    ));
    docks.heard("outer", vec![declare(OUTER, Some(outer))]);
    let moved = DockBarPlacement::Beside(DockBarEdge {
 edge: DockEdge::Left,
 align: DockAlign::Center,
 });
    root_hosts(
        &mut docks,
        vec![DockPlacedBar {
            bar: path(&[], 3),
            owner: Some(path(&[], 1)),
            placement: moved.clone(),
        }],
        Vec::new(),
    );
    let mut framer = declaration("stage", &[(7, "stage")]);
    framer.bars.push(bar(
        8,
        7,
        DockBarPlacement::Over(DockBarEdge {
            edge: DockEdge::Bottom,
            align: DockAlign::Center,
        }),
    ));
    docks.heard("framer", vec![declare(FRAMER, Some(framer))]);
    updates(&mut docks, "outer");
    updates(&mut docks, "framer");

    docks.placed(
        OUTER,
        &[],
        Some(DockedChild {
            child: FRAME_CHILD,
            tab: 1,
            block: FRAMER_BLOCK,
        }),
    );
    assert!(!docks.docked(FRAMER), "its stage stays a frame child");
    assert_eq!(
        delivered_bars(&docks),
        [(path(&[FRAME_CHILD], 8), Some(path(&[], 1)), moved.clone())],
        "its bars join the taken tab's pane where its placer's bar of the same key was, \
         and its placer's bars go"
    );
    let delivered = docks.delivered.get(&(ROOT, OUTER_CHILD)).unwrap();
    assert!(
        delivered.tab(&path(&[FRAME_CHILD], 7)).is_none(),
        "its tabs stay out"
    );
    assert_eq!(
        docks.resolve(ROOT, &path(&[OUTER_CHILD, FRAME_CHILD], 8)),
        Some((FRAMER_BLOCK, 8)),
        "the root places its bar's body"
    );

    let cover = Rect::from_min_size(
        Pos2::new(0.0, 0.0),
        block_plugin_api::Vec2::new(48.0, 300.0),
    );
    root_hosts(
        &mut docks,
        vec![DockPlacedBar {
            bar: path(&[FRAME_CHILD], 8),
            owner: Some(path(&[], 1)),
            placement: DockBarPlacement::Over(DockBarEdge {
                edge: DockEdge::Top,
                align: DockAlign::Center,
            }),
        }],
        vec![DockCover {
            tab: path(&[], 1),
            rect: cover,
        }],
    );
    let framed = hosted(&mut docks, "framer", FRAMER);
    assert_eq!(
        framed.layout.bars,
        [DockPlacedBar {
            bar: path(&[], 8),
            owner: Some(path(&[], 7)),
            placement: DockBarPlacement::Over(DockBarEdge {
                edge: DockEdge::Top,
                align: DockAlign::Center
            }),
        }],
        "it hears where its bar is, owned by its own tab"
    );
    assert_eq!(
        framed.covers,
        [DockCover {
            tab: path(&[], 7),
            rect: cover,
        }]
    );
    let kept = hosted(&mut docks, "outer", OUTER);
    assert_eq!(
        kept.layout.bars,
        [DockPlacedBar {
            bar: path(&[], 3),
            owner: Some(path(&[], 1)),
            placement: moved.clone(),
        }],
        "its placer keeps its hidden bar where it was"
    );

    updates(&mut docks, "shell");
    let drag = DockRequest::MoveBar(DockBarMove {
        bar: path(&[], 8),
        at: Pos2::new(4.0, 4.0),
        stage: DockDragStage::Move,
    });
    docks.heard(
        "framer",
        vec![DockMessage::Request(DockViewRequest {
            view: ViewRef {
                instance: FRAMER,
                view: ViewId::MAIN,
            },
            request: drag,
        })],
    );
    let asked = updates(&mut docks, "shell");
    assert!(
        matches!(
            asked.as_slice(),
            [(ROOT, DockUpdate::ChildRequest(DockChildRequest { child: OUTER_CHILD, owner, .. }))]
                if *owner == [FRAME_CHILD]
        ),
        "its bar's drag reaches the root, got {asked:?}"
    );

    docks.placed(OUTER, &[], None);
    assert_eq!(
        delivered_bars(&docks),
        [(path(&[], 3), Some(path(&[], 1)), moved)],
        "letting the frame go brings its placer's bar back where it was"
    );
    assert!(
        updates(&mut docks, "framer")
            .iter()
            .any(|(to, update)| *to == FRAMER
                && matches!(
                    update,
                    DockUpdate::Hosted(DockHostedView { hosted: None, .. })
                )),
        "and the child draws its bars itself again"
    );
}
