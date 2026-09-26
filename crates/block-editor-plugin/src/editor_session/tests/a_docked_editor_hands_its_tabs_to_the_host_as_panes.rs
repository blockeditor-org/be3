use super::*;
use crate::{ChildBlock, ChildState, ChildTarget, EditorDock};
use beui::reactive::{Func, Text, create_signal, view};
use block_plugin_api::{ChildMode, PaneItem};

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

fn region(region: EditorRegion) -> RegionState {
    RegionState {
        placement: Some(ScreenPlacement {
            screen: block_plugin_api::ScreenId(0),
            instance: EditorInstanceId(0),
            region,
            x: 0,
            y: 0,
            width: 400,
            height: 300,
            scale_factor_millis: 1000,
        }),
        metrics: Some(ViewportMetrics {
            logical_width: 400.0,
            logical_height: 300.0,
            visible_x: 0.0,
            visible_y: 0.0,
            pixel_width: 400,
            pixel_height: 300,
            scale_factor: 1.0,
        }),
        frame: Some(FrameSpec::default()),
        ..Default::default()
    }
}

#[test]
fn a_docked_editor_hands_its_tabs_to_the_host_as_panes() {
    let mut session = EditorSession::new::<DockingApp>(EditorInstanceId(0), Waker::default());
    session.offer_panes(true);
    session.connect(Uuid::new_v4(), Uuid::new_v4());
    session
        .regions
        .insert(EditorRegion::Frame, region(EditorRegion::Frame));
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
                sidebar: 180.0,
            },
            PaneItem::Pane(PaneId(1)),
            PaneItem::Pane(PaneId(2)),
        ]
    );
    assert_eq!(layout.panes[1].title, "Tab 2");

    let pane = EditorRegion::Pane(PaneId(1));
    session.regions.insert(pane, region(pane));
    session.run(pane, 2);
    session.run(pane, 3);

    let placed = &session.regions[&pane].children;
    assert_eq!(
        placed.first().map(|child| child.block_id),
        Some(FILES.into_bytes()),
        "a child block inside a pane is placed on that pane's screen"
    );
    assert!(session.regions[&EditorRegion::Frame].children.is_empty());
}
