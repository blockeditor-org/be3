use super::*;
use crate::EditorDock;
use beui::reactive::{Func, Text, create_signal, view};
use beui::unstyled::SIDEBAR_WIDTH;
use block_plugin_api::{EditorMessage, Message, PaneItem, PaneTree};
use std::cell::RefCell;

thread_local! {
    static CLOSED: RefCell<Vec<TabId>> = const { RefCell::new(Vec::new()) };
    static LAYOUT: RefCell<Option<DockState>> = const { RefCell::new(None) };
}

struct DockingApp;

impl crate::BeuiApp for DockingApp {
    fn view(editor: crate::Editor) -> beui::NodeId {
        let (layout, set_layout) = create_signal(DockState::new([TabId::new(1), TabId::new(2)]));
        view! {
            <EditorDock
                editor={editor}
                state={layout}
                title={Func::new(|tab: TabId| format!("Tab {}", tab.value()))}
                on_change={move |next: DockState| {
                    LAYOUT.with(|layout| *layout.borrow_mut() = Some(next.clone()));
                    set_layout.set(next);
                }}
                on_close={|tab: TabId| CLOSED.with(|closed| closed.borrow_mut().push(tab))}
            >
                {move |tab: TabId| view! {
                    <Text string={format!("tab {}", tab.value())} />
                }}
            </EditorDock>
        }
    }
}

#[test]
fn a_layout_the_host_rearranged_comes_back_as_the_editors_own() {
    let mut session = session::<DockingApp>(Uuid::new_v4());
    session.offer_panes(true);
    regions(&mut session, &[EditorRegion::Frame]);
    session.run(EditorRegion::Frame, 1);
    session.outbound();

    session.arrange_panes(
        1,
        PaneTree {
            items: vec![
                PaneItem::Tabs {
                    count: 1,
                    active: 0,
                    vertical: false,
                    sidebar: SIDEBAR_WIDTH,
                },
                PaneItem::Pane(PaneId(2)),
            ],
        },
        vec![PaneId(1)],
        Some(PaneId(2)),
    );
    session.run(EditorRegion::Frame, 2);

    let layout = LAYOUT
        .with(|layout| layout.borrow().clone())
        .expect("the editor was told its new layout");
    assert_eq!(layout.all_tabs(), vec![TabId::new(2)]);
    assert_eq!(layout.focused_tab(), Some(TabId::new(2)));
    let resent = session
        .outbound()
        .into_iter()
        .find_map(|message| match message {
            Message::Editor(EditorMessage::Panes { layout, .. }) => Some(layout),
            _ => None,
        })
        .expect("the editor answers the arrangement it was given");
    assert_eq!(resent.arrangement, 1);
    assert_eq!(
        resent.panes.len(),
        2,
        "a pane moved out of the dock is still one of the editor's panes"
    );

    session.close_pane(PaneId(1));
    session.run(EditorRegion::Frame, 3);

    assert_eq!(
        CLOSED.with(|closed| closed.borrow().clone()),
        vec![TabId::new(1)]
    );
}
