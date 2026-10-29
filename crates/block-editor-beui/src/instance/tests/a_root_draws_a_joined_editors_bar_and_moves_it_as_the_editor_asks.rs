use super::*;
use beui::dock::unstyled::{DockBarPlacement as Placement, DockEdge as Edge};
use block_plugin_api::{
    ChildContent, ChildId, DockBar, DockBarMove, DockBarPlace, DockBarPlacement, DockChildRequest,
    DockDeclaration, DockDeclaredChild, DockDragStage, DockEdge, DockItem, DockItemKind,
    DockMessage, DockPane, DockRequest, DockTab, DockTabPath, DockTree, DockUpdate, Pos2, Rect,
    Size,
};

fn barred() -> DockDeclaration {
    DockDeclaration {
        tree: DockTree {
            items: vec![
                DockItem {
                    depth: 0,
                    kind: DockItemKind::Pane(DockPane {
                        key: "stage".to_owned(),
                        active: None,
                        vertical: false,
                        icons_only: false,
                        sidebar: 180.0,
                        keep: false,
                        area: None,
                        empty: None,
                        bare: true,
                    }),
                },
                DockItem {
                    depth: 1,
                    kind: DockItemKind::Tab(DockTabPath::own(5)),
                },
            ],
            focus: None,
        },
        tabs: vec![DockTab {
            tab: DockTabPath::own(5),
            key: "5".to_owned(),
            title: "Stage".to_owned(),
            glyph: String::new(),
            closable: false,
            menu: Vec::new(),
        }],
        dialogs: Vec::new(),
        bars: vec![DockBar {
            tab: DockTabPath::own(9),
            key: "tools".to_owned(),
            kind: "canvas/tools".to_owned(),
            owner: DockTabPath::own(5),
            placement: DockBarPlacement::Over(DockEdge::Top),
            size: Size {
                width: 100.0,
                height: 20.0,
            },
        }],
    }
}

fn body(session: &EditorSession, child: ChildId, tab: u64) -> Rect {
    let path = DockTabPath::own(tab).through(child);
    session
        .placed_children(ViewId::MAIN)
        .iter()
        .find(|placed| placed.content == ChildContent::DockTab(path.clone()))
        .map(|placed| placed.rect)
        .unwrap_or_else(|| panic!("the root places the body of tab {tab}"))
}

fn ask(session: &mut EditorSession, child: ChildId, request: DockRequest) {
    session
        .host()
        .receive_dock_updates(vec![DockUpdate::ChildRequest(DockChildRequest {
            child,
            owner: Vec::new(),
            request,
        })]);
    show_docked(session, &[(View::MAIN, 6)], false);
}

fn placement() -> Option<Placement> {
    let layout = LAYOUT
        .with(std::cell::Cell::get)
        .expect("the app made its layout");
    let state = layout.state().get_untracked();
    let bars = state.bars();
    let [bar] = bars.as_slice() else {
        panic!("the root holds the child's one bar, not {bars:?}");
    };
    state.bar_placement(*bar)
}

#[test]
fn a_root_draws_a_joined_editors_bar_and_moves_it_as_the_editor_asks() {
    let mut session = docked_session();
    show_docked(&mut session, &[(View::MAIN, 6)], false);
    let layout = LAYOUT
        .with(std::cell::Cell::get)
        .expect("the app made its layout");
    assert_eq!(
        layout.mode().get_untracked(),
        beui::dock::unstyled::DockMode::Tiled,
        "a wide frame tiles its dock, so bars can be dragged"
    );
    let child = session.placed_children(ViewId::MAIN)[0].child;
    dock_messages(&mut session);
    session
        .host()
        .receive_dock_updates(vec![DockUpdate::Child(DockDeclaredChild {
            child,
            declaration: Some(barred()),
        })]);
    show_docked(&mut session, &[(View::MAIN, 6)], false);
    show_docked(&mut session, &[(View::MAIN, 6)], false);

    assert_eq!(placement(), Some(Placement::Over(Edge::Top)));
    let bar = body(&session, child, 9);
    assert_eq!((bar.width(), bar.height()), (100.0, 20.0));

    ask(
        &mut session,
        child,
        DockRequest::PlaceBar(DockBarPlace {
            bar: DockTabPath::own(9),
            placement: Some(DockBarPlacement::Beside(DockEdge::Left)),
        }),
    );
    assert_eq!(placement(), Some(Placement::Beside(Edge::Left)));
    let hosted = dock_messages(&mut session)
        .into_iter()
        .filter_map(|message| match message {
            DockMessage::Host(host) => match host.hold {
                block_plugin_api::DockHold::Hosted(hosted) => Some(hosted),
                _ => None,
            },
            _ => None,
        })
        .last()
        .expect("the root hosts its child again");
    assert_eq!(hosted.layout.bars[0].bar, DockTabPath::own(9));
    assert_eq!(
        hosted.layout.bars[0].placement,
        DockBarPlacement::Beside(DockEdge::Left)
    );

    let bar = body(&session, child, 9);
    let stage = body(&session, child, 5);
    let target = beui::pos2(stage.center().x, stage.max.y - 4.0);
    let moved = |at: beui::Pos2, stage| {
        DockRequest::MoveBar(DockBarMove {
            bar: DockTabPath::own(9),
            at: Pos2::new(at.x, at.y),
            stage,
        })
    };
    ask(
        &mut session,
        child,
        moved(beui::pos2(2.0, 2.0), DockDragStage::Move),
    );
    ask(
        &mut session,
        child,
        moved(target - bar.min.to_vec2(), DockDragStage::Move),
    );
    assert_eq!(
        placement(),
        Some(Placement::Beside(Edge::Left)),
        "the bar stays put while the root carries it"
    );
    ask(
        &mut session,
        child,
        moved(target - bar.min.to_vec2(), DockDragStage::Drop),
    );
    assert_eq!(placement(), Some(Placement::Beside(Edge::Bottom)));
}
