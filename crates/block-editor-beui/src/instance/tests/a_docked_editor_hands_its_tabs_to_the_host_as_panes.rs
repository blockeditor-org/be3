use super::*;
use crate::{ChildBlock, ChildState, ChildTarget, EditorDock};
use beui::reactive::{Func, Text, create_signal, view};
use beui::unstyled::SIDEBAR_WIDTH;
use block_plugin_api::{ChildMode, EditorMessage, Message, PaneItem};

const FILES: Uuid = Uuid::from_u128(1);
const FILES_TYPE: Uuid = Uuid::from_u128(2);

struct DockingApp;

impl crate::BeuiApp for DockingApp {
    fn view(editor: crate::Editor) -> beui::NodeId {
        let (layout, set_layout) = create_signal(DockState::new([TabId::new(1), TabId::new(2)]));
        let child = editor.clone();
        view! {
            <EditorDock
                editor={editor}
                state={layout}
                title={Func::new(|tab: TabId| format!("Tab {}", tab.value()))}
                on_change={move |next: DockState| set_layout.set(next)}
                on_close={|_: TabId| {}}
            >
                {move |tab: TabId| match tab.value() {
                    1 => view! {
                        <ChildBlock
                            editor={child.clone()}
                            block={Some(ChildTarget::new(FILES, FILES_TYPE))}
                            mode=ChildMode::Live
                            on_state={move |_: ChildState| {}}
                        />
                    },
                    _ => view! {
                        <Text string="second" />
                    },
                }}
            </EditorDock>
        }
    }
}

#[test]
fn a_docked_editor_hands_its_tabs_to_the_host_as_panes() {
    let mut session = session::<DockingApp>(Uuid::new_v4());
    session.offer_panes(true);
    regions(&mut session, &[EditorRegion::Frame]);
    session.run(EditorRegion::Frame, 1);

    let layout = session
        .outbound()
        .into_iter()
        .find_map(|message| match message {
            Message::Editor(EditorMessage::Panes { layout, .. }) => Some(layout),
            _ => None,
        })
        .expect("a docked editor describes its panes to the host");
    assert_eq!(
        layout.tree.items,
        vec![
            PaneItem::Tabs {
                count: 2,
                active: 0,
                vertical: false,
                sidebar: SIDEBAR_WIDTH,
            },
            PaneItem::Pane(PaneId(1)),
            PaneItem::Pane(PaneId(2)),
        ]
    );
    assert_eq!(layout.panes[1].title, "Tab 2");

    let pane = EditorRegion::Pane(PaneId(1));
    regions(&mut session, &[EditorRegion::Frame, pane]);
    session.run(pane, 2);
    session.run(pane, 3);

    assert_eq!(
        session
            .placed_children(pane)
            .first()
            .map(|child| child.block_id),
        Some(FILES.into_bytes()),
        "a child block inside a pane is placed on that pane's screen"
    );
    assert!(session.placed_children(EditorRegion::Frame).is_empty());
}
