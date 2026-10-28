use super::*;
use block_plugin_api::{DockMessage, DockTabPath};

struct NestingApp;

#[beui::reactive::component]
impl crate::BeuiApp for NestingApp {
    #[beui::reactive::component]
    fn View(editor: crate::Editor) -> beui::styled::Root {
        use beui::dock::styled::{DockDialog, DockPane, DockTab, Docking, DockingLayout};
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
                                    <DockTab id=11u64 title="Inner 11">
                                        <beui::styled::Frame />
                                    </DockTab>
                                </DockPane>
                            </Docking>
                        </DockTab>
                        <DockTab id=2u64 title="Two">
                            <beui::styled::Frame />
                        </DockTab>
                    </DockPane>
                    <DockDialog id=3u64 parent=2u64 title="Dialog" size={beui::vec2(200.0, 100.0)}>
                        <beui::styled::Frame />
                    </DockDialog>
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
fn a_docked_editors_nested_dock_and_dialog_join_its_declaration() {
    let mut session = session::<NestingApp>(Uuid::new_v4());
    show_docked(&mut session, &[(EditorRegion::Frame, 0)], true);
    let first = declared(&mut session).expect("the docked editor declares its dock");
    let dialog = first
        .dialog(&DockTabPath::own(3))
        .expect("the dialog is declared for the root to present");
    assert_eq!(dialog.parent, DockTabPath::own(2));
    assert_eq!(dialog.title, "Dialog");

    session
        .host()
        .receive_dock_updates(vec![hosted_tabs(&[1, 2], &[1], Some(1))]);
    let shown = [(EditorRegion::Frame, 0), (EditorRegion::Tab(1), 1)];
    for _ in 0..3 {
        show_docked(&mut session, &shown, true);
    }

    let joined = declared(&mut session).expect("the nested dock joins the declaration");
    let titles: Vec<&str> = joined.tabs.iter().map(|tab| tab.title.as_str()).collect();
    assert!(
        titles.contains(&"Inner 10") && titles.contains(&"Inner 11"),
        "the nested dock's tabs are declared: {titles:?}"
    );
    assert!(
        !titles.contains(&"One"),
        "the tab that holds the nested dock gives way to it: {titles:?}"
    );
    let nodes = joined.nest().expect("the declaration is valid");
    assert!(
        nodes
            .panes()
            .iter()
            .any(|pane| pane.key == "joined:1/inner"),
        "the nested dock's pane is spliced in as a group"
    );
    joined
        .validate_own()
        .expect("the plugin still declares only its own tabs");
}
