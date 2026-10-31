use super::*;
use block_plugin_api::DockMessage;

struct NestedBarApp;

#[beui::reactive::component]
impl crate::BeuiApp for NestedBarApp {
    #[beui::reactive::component]
    fn View(editor: crate::Editor) -> beui::styled::Root {
        use beui::dock::styled::{
            DockAlign, DockBar, DockBarPlacement, DockEdge, DockPane, DockTab, Docking,
            DockingLayout,
        };
        use beui::reactive::*;
        let _ = editor;
        let outer = DockingLayout::<u64>::new();
        let inner = DockingLayout::<u64>::new();
        view! {
            <beui::styled::Root>
                <Docking layout=outer>
                    <DockPane id="tabs">
                        <DockTab id=1u64 title="One">
                            <Docking layout=inner>
                                <DockPane id="inner">
                                    <DockTab id=10u64 title="Inner 10">
                                        <beui::styled::Frame />
                                    </DockTab>
                                </DockPane>
                                <DockBar
                                    id=20u64
                                    placement={DockBarPlacement::Beside(DockEdge::Bottom, DockAlign::Center)}
                                >
                                    <beui::styled::Frame />
                                </DockBar>
                            </Docking>
                        </DockTab>
                        <DockTab id=2u64 title="Two">
                            <beui::styled::Frame />
                        </DockTab>
                    </DockPane>
                </Docking>
            </beui::styled::Root>
        }
    }
}

fn declared(session: &mut EditorSession) -> Option<block_plugin_api::DockDeclaration> {
    dock_messages(session)
        .into_iter()
        .filter_map(|message| match message {
            DockMessage::Declare(declare) => declare.declaration,
            _ => None,
        })
        .last()
}

#[test]
fn a_nested_docks_own_bar_is_declared_with_the_tab_its_group_shows() {
    let mut session = session::<NestedBarApp>(Uuid::new_v4());
    show_docked(&mut session, &[(View::MAIN, 0)], true);
    declared(&mut session).expect("the docked editor declares its dock");
    session
        .host()
        .receive_dock_updates(vec![hosted_tabs(&[1, 2], &[1], Some(1))]);
    let shown = [(View::MAIN, 0), (View::dock(ViewId(1)), 1)];
    for _ in 0..3 {
        show_docked(&mut session, &shown, true);
    }

    let joined = declared(&mut session).expect("the nested dock joins the declaration");
    let titles: Vec<&str> = joined.tabs.iter().map(|tab| tab.title.as_str()).collect();
    assert!(
        !titles.contains(&"One"),
        "the tab that holds the nested dock gives way to it: {titles:?}"
    );
    let [bar] = joined.bars.as_slice() else {
        panic!("the nested dock's bar is declared: {:?}", joined.bars);
    };
    let owner = bar.owner.clone().expect("the bar belongs to a tab");
    assert_eq!(
        joined.tab(&owner).map(|tab| tab.title.as_str()),
        Some("Inner 10"),
        "the bar belongs to the tab its group shows, not to the tab the group replaced"
    );
    joined
        .validate_own()
        .expect("a declaration with the nested dock's bar is one the root accepts");
}
