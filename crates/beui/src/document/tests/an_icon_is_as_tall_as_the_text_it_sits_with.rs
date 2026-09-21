use super::*;
use crate::icons::ICON_FOLDER;
use crate::reactive::{Align, Direction, Frame, List, NodeRef, view};
use crate::styled::{Body, Icon};

const LABEL: &str = "Folder";

#[test]
fn an_icon_is_as_tall_as_the_text_it_sits_with() {
    let text = height(|| {
        view! {
            <Body content=LABEL />
        }
    });
    let icon = height(|| {
        view! {
            <Icon glyph=ICON_FOLDER />
        }
    });
    let row = height(|| {
        view! {
            <List direction=Direction::Horizontal align=Align::Center spacing=4.0>
                <Icon glyph=ICON_FOLDER />
                <Body content=LABEL />
            </List>
        }
    });
    assert_eq!(icon, text);
    assert_eq!(row, text);
}

fn height(content: impl FnOnce() -> NodeId + 'static) -> f32 {
    let frame = NodeRef::new();
    let placed = frame.clone();
    let document = build(move || {
        view! {
            <List spacing=0.0>
                <Frame @node_ref=&placed>{content()}</Frame>
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
