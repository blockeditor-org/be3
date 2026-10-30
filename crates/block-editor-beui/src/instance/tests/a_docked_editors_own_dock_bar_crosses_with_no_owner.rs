use super::*;
use beui::dock::unstyled::{DockAlign as Align, DockBarPlacement as Placement, DockEdge as Edge};
use block_plugin_api::{
    DockAlign, DockBarEdge,
    DockBarPlacement, DockEdge, DockHosted, DockHostedView, DockItem, DockItemKind, DockLayout,
    DockMessage, DockPane, DockPlacedBar, DockTabPath, DockTree, DockUpdate,
};

fn hosted(bar: u64) -> DockUpdate {
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
                    owner: None,
                    placement: DockBarPlacement::Beside(DockBarEdge {
 edge: DockEdge::Left,
 align: DockAlign::Center,
 }),
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

#[test]
fn a_docked_editors_own_dock_bar_crosses_with_no_owner() {
    let mut session = dock_barred_session();
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
        .expect("a docked editor declares its dock's own bar");
    let [bar] = declaration.bars.as_slice() else {
        panic!("the dock declares its own bar, not {:?}", declaration.bars);
    };
    assert_eq!(bar.owner, None, "a bar of the dock itself names no tab");
    assert_eq!(bar.placement, DockBarPlacement::Beside(DockBarEdge {
 edge: DockEdge::Bottom,
 align: DockAlign::Center,
 }));
    let id = bar.tab.tab;

    session.host().receive_dock_updates(vec![hosted(id)]);
    let shown = [(View::MAIN, 0), (View::dock(ViewId(id)), 1)];
    show_docked(&mut session, &shown, true);
    show_docked(&mut session, &shown, true);

    let control = BAR
        .with(std::cell::Cell::get)
        .expect("the bar's body is built in its own region");
    assert!(control.is_hosted());
    assert_eq!(control.placement(), Placement::Beside(Edge::Left, Align::Center));
    assert_eq!(control.direction(), beui::Direction::Vertical);
    let layout = LAYOUT
        .with(std::cell::Cell::get)
        .expect("the app made its layout");
    let state = layout.state().get_untracked();
    assert_eq!(
        state.bars_of(None),
        [beui::dock::unstyled::TabId::new(id)],
        "the joined dock holds the root's placement as its own dock's bar"
    );
}
