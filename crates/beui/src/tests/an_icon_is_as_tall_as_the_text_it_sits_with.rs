use super::*;
use crate::TextAlign;
use crate::icons::ICON_FOLDER;
use crate::primitives::Frame;
use crate::reactive::*;
use crate::styled::Text;
use crate::styled::*;
use crate::primitives::List;

const LABEL: &str = "Folder";

#[test]
fn an_icon_is_as_tall_as_the_text_it_sits_with() {
    let _root = crate::reactive::RootScope::new();
    let text = height(|| {
        view! {
            <Text>{LABEL}</Text>
        }
    });
    let icon = height(|| {
        view! {
            <Text align=TextAlign::Center>
                <Icon glyph=ICON_FOLDER />
            </Text>
        }
    });
    let row = height(|| {
        view! {
            <List direction=Direction::Horizontal align=Align::Center spacing=4.0>
                <Text align=TextAlign::Center>
                    <Icon glyph=ICON_FOLDER />
                </Text>
                <Text>{LABEL}</Text>
            </List>
        }
    });
    assert_eq!(icon, text);
    assert_eq!(row, text);
}

fn height(content: impl FnOnce() -> NodeId + 'static) -> f32 {
    let frame = NodeRef::new();
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <Frame @node_ref=&frame>{content()}</Frame>
            </List>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    harness
        .document()
        .node_rect(frame.get())
        .expect("the content was laid out")
        .height()
}
