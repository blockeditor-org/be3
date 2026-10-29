use super::*;
use beui::dock::unstyled::{DockBarPlacement as Placement, DockEdge as Edge};
use block_plugin_api::{
    DockBarMove, DockBarPlace, DockBarPlacement, DockDragStage, DockEdge, DockHosted,
    DockHostedView, DockItem, DockItemKind, DockLayout, DockMessage, DockPane, DockPlacedBar,
    DockRequest, DockTabPath, DockTree, DockUpdate,
};

fn hosted(bar: u64, placement: DockBarPlacement) -> DockUpdate {
    let stage = DockTabPath::own(1);
    DockUpdate::Hosted(DockHostedView {
        view: ViewId::MAIN,
        hosted: Some(DockHosted {
            layout: DockLayout {
                tree: DockTree {
                    items: vec![
                        DockItem {
                            depth: 0,
                            kind: DockItemKind::Pane(DockPane {
                                key: "stage".to_owned(),
                                active: Some(0),
                                vertical: false,
                                icons_only: false,
                                sidebar: 180.0,
                                keep: false,
                                area: Some("stage".to_owned()),
                                empty: None,
                                bare: true,
                            }),
                        },
                        DockItem {
                            depth: 1,
                            kind: DockItemKind::Tab(stage.clone()),
                        },
                    ],
                    focus: Some(stage.clone()),
                },
                homes: Vec::new(),
                unlocked: Vec::new(),
                bars: vec![DockPlacedBar {
                    bar: DockTabPath::own(bar),
                    owner: stage.clone(),
                    placement,
                }],
                outside: Vec::new(),
            },
            tabs: vec![stage.clone()],
            focused: Some(stage.clone()),
            shown: None,
            visible: vec![stage],
            fullscreen: None,
            stacked: false,
            covers: Vec::new(),
        }),
    })
}

fn press(at: beui::Pos2) -> beui::PointerPress {
    beui::PointerPress {
        window: beui::WindowId::MAIN,
        pos: at,
        local: at,
        size: beui::Vec2::ZERO,
        fraction: beui::Vec2::ZERO,
        clicks: 1,
        modifiers: beui::Modifiers::default(),
        touch: false,
        pen: None,
        predicted: None,
        time: beui::reactive::now(),
    }
}

fn requests(session: &mut EditorSession) -> Vec<DockRequest> {
    dock_messages(session)
        .into_iter()
        .filter_map(|message| match message {
            DockMessage::Request(request) => Some(request.request),
            _ => None,
        })
        .collect()
}

#[test]
fn a_docked_editors_bar_declares_its_size_and_asks_the_root_to_place_and_move_it() {
    let mut session = barred_session();
    show_docked(&mut session, &[(View::MAIN, 0)], true);
    let declaration = dock_messages(&mut session)
        .into_iter()
        .find_map(|message| match message {
            DockMessage::Declare(declare) => declare.declaration,
            _ => None,
        })
        .expect("the docked editor declares its dock");
    declaration
        .validate_own()
        .expect("a docked editor declares only its own bars");
    let [bar] = declaration.bars.as_slice() else {
        panic!("the dock declares its one bar, not {:?}", declaration.bars);
    };
    assert_eq!(bar.key, "3");
    let (block_type, key) = bar.kind.split_once('/').expect("a bar's kind names its type");
    assert!(
        Uuid::try_parse(block_type).is_ok() && key == "3",
        "a bar's kind is the editor's block type and the bar's key: {}",
        bar.kind
    );
    assert_eq!(bar.owner, DockTabPath::own(1));
    assert_eq!(bar.placement, DockBarPlacement::Over(DockEdge::Top));
    let id = bar.tab.tab;
    let pane = declaration.nest().unwrap();
    assert!(pane.panes()[0].bare, "the bare pane says so");

    session
        .host()
        .receive_dock_updates(vec![hosted(id, DockBarPlacement::Beside(DockEdge::Right))]);
    let shown = [(View::MAIN, 0), (View::dock(ViewId(id)), 1)];
    show_docked(&mut session, &shown, true);
    show_docked(&mut session, &shown, true);

    let control = BAR
        .with(std::cell::Cell::get)
        .expect("the bar's body is built in its own region");
    assert!(control.is_hosted());
    assert_eq!(control.placement(), Placement::Beside(Edge::Right));
    assert_eq!(control.direction(), beui::Direction::Vertical);
    let sized = dock_messages(&mut session)
        .into_iter()
        .filter_map(|message| match message {
            DockMessage::Declare(declare) => declare.declaration,
            _ => None,
        })
        .last()
        .expect("the editor declares again once its bar is measured");
    let size = sized.bars[0].size;
    assert_eq!((size.width, size.height), (24.0, 120.0));

    control.place(Placement::Beside(Edge::Bottom));
    control.pop_out();
    control.begin_move(press(beui::pos2(4.0, 6.0)));
    control.begin_move(press(beui::pos2(30.0, 60.0)));
    control.end_move();
    show_docked(&mut session, &shown, true);
    let moved = |at: (f32, f32), stage| {
        DockRequest::MoveBar(DockBarMove {
            bar: DockTabPath::own(id),
            at: block_plugin_api::Pos2::new(at.0, at.1),
            stage,
        })
    };
    assert_eq!(
        requests(&mut session),
        [
            DockRequest::PlaceBar(DockBarPlace {
                bar: DockTabPath::own(id),
                placement: Some(DockBarPlacement::Beside(DockEdge::Bottom)),
            }),
            DockRequest::PlaceBar(DockBarPlace {
                bar: DockTabPath::own(id),
                placement: None,
            }),
            moved((4.0, 6.0), DockDragStage::Move),
            moved((30.0, 60.0), DockDragStage::Move),
            moved((30.0, 60.0), DockDragStage::Drop),
        ]
    );
}
