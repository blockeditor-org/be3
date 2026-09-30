use super::*;
use crate::{EditorDock, NARROW_WIDTH};
use beui::reactive::{Func, Text, component, create_memo, create_signal, view};
use beui::unstyled::{Container, DockMode, narrower_than};
use block_plugin_api::{EditorMessage, Message};

struct StackingApp;

impl crate::BeuiApp for StackingApp {
    fn view(editor: crate::Editor) -> beui::NodeId {
        view! {
            <Container>
                {move |_| {
                    let editor = editor.clone();
                    view! {
                        <StackingDock editor={editor} />
                    }
                }}
            </Container>
        }
    }
}

#[component]
fn StackingDock(editor: crate::Editor) -> beui::NodeId {
    let (layout, set_layout) = create_signal(DockState::new([TabId::new(1), TabId::new(2)]));
    let narrow = narrower_than(NARROW_WIDTH);
    let mode = create_memo(move || match narrow.get() {
        true => DockMode::Stacked,
        false => DockMode::Tiled,
    });
    view! {
        <EditorDock
            editor={editor}
            state={layout}
            mode={mode}
            title={Func::new(|tab: TabId| format!("Tab {}", tab.value()))}
            on_change={move |next: DockState| set_layout.set(next)}
            on_close={|_: TabId| {}}
        >
            {move |tab: TabId| view! {
                <Text string={format!("content {}", tab.value())} />
            }}
        </EditorDock>
    }
}

fn published(session: &mut EditorSession) -> Vec<bool> {
    session
        .outbound()
        .into_iter()
        .filter_map(|message| match message {
            Message::Editor(EditorMessage::Panes { layout, .. }) => Some(layout.is_some()),
            _ => None,
        })
        .collect()
}

#[test]
fn a_stacked_dock_takes_its_panes_back_from_the_host() {
    let mut session = session::<StackingApp>(Uuid::new_v4());
    session.offer_panes(true);
    frame(&mut session, None, TopBar::Hidden);
    session.run(EditorRegion::Frame, 1);
    session.run(EditorRegion::Frame, 2);
    assert_eq!(published(&mut session), vec![true]);

    regions(&mut session, &[EditorRegion::Frame]);
    session.run(EditorRegion::Frame, 3);
    session.run(EditorRegion::Frame, 4);

    assert_eq!(
        published(&mut session),
        vec![false],
        "a dock stacked for a phone draws its tabs itself and withdraws its panes"
    );

    frame(&mut session, None, TopBar::Hidden);
    session.run(EditorRegion::Frame, 5);
    session.run(EditorRegion::Frame, 6);

    assert_eq!(
        published(&mut session),
        vec![true],
        "tiling again hands the tabs back to the host"
    );
}
