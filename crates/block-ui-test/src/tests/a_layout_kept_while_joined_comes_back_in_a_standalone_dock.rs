use super::*;
use beui::dock::styled::{DockAlign as Align, DockBarPlacement as Placement, DockEdge as Edge};
use beui::dock::unstyled::DockOutsidePlace as Outside;
use block_plugin_api::{
    DockAlign, DockBarEdge, DockBarPlacement, DockEdge, DockHosted, DockHostedView, DockItem,
    DockItemKind, DockLayout, DockMessage, DockOutside, DockOutsidePlace, DockPane, DockPlacedBar,
    DockTabPath, DockTree, DockUpdate, Rect, ViewId,
};
use std::collections::HashMap;

fn declared(test: &mut BeuiTest<StandaloneApp>) -> HashMap<String, u64> {
    let declaration = test
        .take_dock_messages()
        .into_iter()
        .filter_map(|message| match message {
            DockMessage::Declare(declare) => declare.declaration,
            _ => None,
        })
        .last()
        .expect("the docked editor declares its dock");
    declaration
        .tabs
        .iter()
        .map(|tab| (tab.key.clone(), tab.tab.tab))
        .chain(
            declaration
                .bars
                .iter()
                .map(|bar| (bar.key.clone(), bar.tab.tab)),
        )
        .collect()
}

fn stage_pane() -> DockItem {
    DockItem {
        depth: 0,
        kind: DockItemKind::Pane(DockPane {
            key: "stage".to_owned(),
            active: Some(0),
            vertical: false,
            icons_only: false,
            sidebar: 180.0,
            keep: false,
            area: None,
            empty: None,
            bare: true,
        }),
    }
}

#[test]
fn a_layout_kept_while_joined_comes_back_in_a_standalone_dock() {
    let (mut joined, display, _) = standalone(DisplaySettingsContent::default(), true);
    let ids = declared(&mut joined);
    let (stage, notes, bar) = (ids["1"], ids["2"], ids["3"]);
    let window = Rect::from_min_size(beui::pos2(40.0, 30.0), beui::vec2(200.0, 150.0));
    joined.receive_dock_updates(vec![DockUpdate::Hosted(DockHostedView {
        view: ViewId::MAIN,
        hosted: Some(DockHosted {
            layout: DockLayout {
                tree: DockTree {
                    items: vec![
                        stage_pane(),
                        DockItem {
                            depth: 1,
                            kind: DockItemKind::Tab(DockTabPath::own(stage)),
                        },
                    ],
                    focus: None,
                },
                homes: Vec::new(),
                unlocked: Vec::new(),
                bars: vec![DockPlacedBar {
                    bar: DockTabPath::own(bar),
                    owner: Some(DockTabPath::own(stage)),
                    placement: DockBarPlacement::Beside(DockBarEdge {
                        edge: DockEdge::Left,
                        align: DockAlign::Center,
                    }),
                    hidden: false,
                }],
                outside: vec![DockOutside {
                    tab: DockTabPath::own(notes),
                    place: DockOutsidePlace::Window(window),
                }],
            },
            tabs: vec![DockTabPath::own(stage), DockTabPath::own(notes)],
            focused: None,
            shown: None,
            visible: vec![DockTabPath::own(stage), DockTabPath::own(notes)],
            fullscreen: None,
            stacked: false,
            covers: Vec::new(),
        }),
    })]);
    kept_dock(&joined, display);

    let kept = joined.content::<DisplaySettingsContent>(Some(display));
    let (_alone, _, layout) = standalone(kept, false);
    assert_eq!(
        layout.bar_placement(&3),
        Some(Placement::Beside(Edge::Left, Align::Center)),
        "a bar moved while joined is where it was moved to"
    );
    let notes = layout.tab_of(&2).expect("the notes tab is declared");
    let stage = layout.tab_of(&1).expect("the stage tab is declared");
    let spec = layout.state().get_untracked().spec();
    let windowed = spec.windows.iter().find(|held| {
        held.root.as_ref().is_some_and(|root| {
            matches!(root, beui::dock::unstyled::DockSpecNode::Pane(pane)
                if pane.entries.contains(&beui::dock::unstyled::DockSpecEntry::Tab(notes)))
        })
    });
    assert_eq!(
        windowed.map(|held| held.rect),
        Some(window),
        "a tab moved out into a window while joined opens in a window of its own"
    );
    assert!(
        spec.outside
            .iter()
            .all(|outside| !matches!(outside.place, Outside::Window { .. })),
        "the window is no longer kept as outside the dock"
    );
    let main = format!("{:?}", spec.main);
    assert!(
        main.contains(&format!("Tab({stage:?})")),
        "the stage stays in the main area: {main}"
    );
}
