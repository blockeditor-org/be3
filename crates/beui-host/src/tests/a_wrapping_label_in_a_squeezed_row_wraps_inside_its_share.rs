use super::*;
use crate::primitives::*;

const LABEL: &str = "Notifications enabled";

#[test]
fn a_wrapping_label_in_a_squeezed_row_wraps_inside_its_share() {
    let _root = crate::reactive::RootScope::new();
    let text = NodeRef::new();
    let document = build_node(move || {
        view! {
            <List direction=Direction::Horizontal spacing=0.0>
                <ListItem grow=1.0>
                    <Text @node_ref=&text>{LABEL}</Text>
                </ListItem>
                <Frame width=140.0 height=20.0 />
            </List>
        }
    });
    let mut harness = Harness::sized(document, vec2(200.0, 100.0));
    harness.frame(Vec::new());
    let rect = harness
        .document()
        .node_rect(text.get())
        .expect("the label was placed");
    assert!(
        rect.width() <= 60.0,
        "the label takes only what the row leaves it, never its longest word: {}",
        rect.width()
    );
    let node = kind_of::<TextNode>(harness.document(), text.get());
    let layout = harness
        .document()
        .text_layout(node)
        .expect("the text was laid out");
    assert!(layout.size.x <= rect.width());
    let lines: Vec<&str> = layout
        .lines
        .iter()
        .map(|line| &LABEL[line.range.clone()])
        .collect();
    assert!(
        lines.len() > 2 && lines[0].starts_with("Notif") && !lines[0].contains(' '),
        "\"Notifications\" breaks inside itself to fit: {lines:?}"
    );
}
