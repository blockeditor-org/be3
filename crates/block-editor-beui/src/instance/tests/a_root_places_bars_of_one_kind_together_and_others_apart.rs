use super::*;
use block_plugin_api::{
    DockBar, DockBarPlace, DockBarPlacement, DockChildRequest, DockDeclaration, DockDeclaredChild,
    DockEdge, DockItem, DockItemKind, DockPane, DockRequest, DockTab, DockTabPath, DockTree,
    DockUpdate, Size,
};

fn bar(tab: u64, key: &str, kind: &str) -> DockBar {
    DockBar {
        tab: DockTabPath::own(tab),
        key: key.to_owned(),
        kind: kind.to_owned(),
        owner: DockTabPath::own(5),
        placement: DockBarPlacement::Over(DockEdge::Top),
        size: Size {
            width: 100.0,
            height: 20.0,
        },
    }
}

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
        bars: vec![
            bar(7, "a/canvas/tools", "canvas/tools"),
            bar(8, "b/canvas/tools", "canvas/tools"),
            bar(9, "c/deck/tools", "deck/tools"),
        ],
    }
}

#[test]
fn a_root_places_bars_of_one_kind_together_and_others_apart() {
    let mut session = docked_session();
    show_docked(&mut session, &[(View::MAIN, 6)], false);
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

    session
        .host()
        .receive_dock_updates(vec![DockUpdate::ChildRequest(DockChildRequest {
            child,
            owner: Vec::new(),
            request: DockRequest::PlaceBar(DockBarPlace {
                bar: DockTabPath::own(7),
                placement: Some(DockBarPlacement::Beside(DockEdge::Left)),
            }),
        })]);
    show_docked(&mut session, &[(View::MAIN, 6)], false);

    let layout = LAYOUT
        .with(std::cell::Cell::get)
        .expect("the app made its layout");
    let state = layout.state().get_untracked();
    let placements: Vec<_> = state
        .bars()
        .into_iter()
        .map(|bar| state.bar_placement(bar))
        .collect();
    use beui::dock::unstyled::{DockBarPlacement as Placement, DockEdge as Edge};
    assert_eq!(
        placements,
        vec![
            Some(Placement::Beside(Edge::Left)),
            Some(Placement::Beside(Edge::Left)),
            Some(Placement::Over(Edge::Top)),
        ],
        "a bar of the same kind follows, one of the same key but another kind stays"
    );
}
