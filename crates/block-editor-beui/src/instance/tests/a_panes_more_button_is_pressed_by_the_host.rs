use super::*;
use crate::EditorDock;
use beui::reactive::{ClickCallback, Func, Text, create_signal, view};
use beui::unstyled::dock_more;
use block_plugin_api::{EMPTY_PANE, EditorMessage, Message};
use std::cell::Cell;

thread_local! {
    static PRESSED: Cell<u32> = const { Cell::new(0) };
}

struct MoreApp;

impl crate::BeuiApp for MoreApp {
    fn view(editor: crate::Editor) -> beui::NodeId {
        let (layout, set_layout) = create_signal(DockState::new([TabId::new(1), TabId::new(2)]));
        view! {
            <EditorDock
                editor={editor}
                state={layout}
                home={Some(TabId::new(1))}
                title={Func::new(|tab: TabId| format!("Tab {}", tab.value()))}
                on_change={move |next: DockState| set_layout.set(next)}
                on_close={|_: TabId| {}}
            >
                {move |tab: TabId| {
                    if tab.value() == 2 {
                        dock_more(ClickCallback::new(|| {
                            PRESSED.with(|pressed| pressed.set(pressed.get() + 1));
                        }));
                    }
                    view! {
                        <Text string={format!("tab {}", tab.value())} />
                    }
                }}
            </EditorDock>
        }
    }
}

fn latest(session: &mut EditorSession) -> Option<PaneLayout> {
    session
        .outbound()
        .into_iter()
        .filter_map(|message| match message {
            Message::Editor(EditorMessage::Panes { layout, .. }) => layout,
            _ => None,
        })
        .next_back()
}

#[test]
fn a_panes_more_button_is_pressed_by_the_host() {
    let mut session = session::<MoreApp>(Uuid::new_v4());
    session.offer_panes(true);
    regions(&mut session, &[EditorRegion::Frame]);
    session.run(EditorRegion::Frame, 1);
    let first = latest(&mut session).expect("the editor describes its panes");
    assert_eq!(first.home, Some(PaneId(1)));
    assert!(first.empty, "the dock offers what an empty pane shows");

    let second = EditorRegion::Pane(PaneId(2));
    let empty = EditorRegion::Pane(EMPTY_PANE);
    regions(&mut session, &[EditorRegion::Frame, second, empty]);
    session.run(second, 2);
    session.run(empty, 2);
    session.run(EditorRegion::Frame, 3);

    let described = latest(&mut session).expect("a pane asking for More is described again");
    let wants = |pane: u64| {
        described
            .panes
            .iter()
            .find(|info| info.pane == PaneId(pane))
            .is_some_and(|info| info.more)
    };
    assert!(wants(2), "the pane that asked for a More button has one");
    assert!(!wants(1));

    session.pane_more(PaneId(2));
    session.run(EditorRegion::Frame, 4);

    assert_eq!(
        PRESSED.with(Cell::get),
        1,
        "the host's press reaches the pane"
    );
}
