use block_editor_beui::beui::styled::Root;
use block_editor_beui::{BarAction, TopBar};

use super::*;

struct BarApp;

#[component]
impl BeuiApp for BarApp {
    #[component]
    fn View(editor: Editor) -> Root {
        view! {
            <Root>
                <BarParent editor={editor} />
            </Root>
        }
    }
}

#[component]
fn BarParent(editor: Editor) -> NodeId {
    let target = Some(ChildTarget::new(SLIDE, SLIDE_TYPE));
    let heard = create_signal(String::from("nothing"));
    view! {
        <List spacing=Space::None>
            <Text @test_id={"bar.heard"}>{heard}</Text>
            <Frame width=Size::Popover height={Size::Custom(180.0)}>
                <ChildBlock
                    editor={editor}
                    block={target}
                    mode=ChildMode::Live
                    own_frame=true
                    top_bar={TopBar::Phone}
                    on_bar={move |action: BarAction| heard.set(format!("{action:?}"))}
                />
            </Frame>
        </List>
    }
}

#[test]
fn a_child_block_hears_what_its_childs_bar_asked_for() {
    let mut test = BeuiTest::<BarApp>::editing(Uuid::new_v4());
    test.run();
    let placement = test.children()[0].clone();
    assert_eq!(placement.top_bar, TopBar::Phone);

    test.child_bar(placement.child, BarAction::Details);
    test.run();

    let node = test
        .document()
        .find_test_id("bar.heard")
        .expect("the heard text is in the tree");
    assert_eq!(
        test.document().node_detail(node).as_deref(),
        Some("\"Details\"")
    );
}
