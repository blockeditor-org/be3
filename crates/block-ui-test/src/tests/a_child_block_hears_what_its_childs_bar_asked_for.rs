use beui::reactive::{List, create_signal};
use block_editor_beui::{BarAction, TopBar};

use super::*;

struct BarApp;

impl BeuiApp for BarApp {
    fn view(editor: Editor) -> NodeId {
        view! {
            <BarParent editor={editor} />
        }
    }
}

#[component]
fn BarParent(editor: Editor) -> NodeId {
    let target = Some(ChildTarget::new(SLIDE, SLIDE_TYPE));
    let (heard, hear) = create_signal(String::from("nothing"));
    view! {
        <List spacing=0.0>
            <Text string={heard} @test_id={"bar.heard"} />
            <Frame width=320.0 height=180.0>
                <ChildBlock
                    editor={editor}
                    block={target}
                    mode=ChildMode::Live
                    own_frame=true
                    top_bar={TopBar::Phone { more: true }}
                    on_bar={move |action: BarAction| hear.set(format!("{action:?}"))}
                />
            </Frame>
        </List>
    }
}

#[test]
fn a_child_block_hears_what_its_childs_bar_asked_for() {
    let host = EditorHost::default();
    host.set_editable(true);
    let mut test = BeuiTest::<BarApp>::new(Editor::new(host, Uuid::new_v4()));
    test.run();
    let placement = test.children()[0];
    assert_eq!(placement.top_bar, TopBar::Phone { more: true });

    test.child_bar(placement.child, BarAction::CloseMore);
    test.run();

    let node = test
        .document()
        .find_test_id("bar.heard")
        .expect("the heard text is in the tree");
    assert_eq!(
        test.document().node_detail(node).as_deref(),
        Some("\"CloseMore\"")
    );
}
