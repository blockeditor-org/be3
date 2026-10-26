use super::*;
use crate::primitives::*;
use crate::reactive::*;

const LABEL: &str = "Notifications enabled";

#[test]
fn a_wrapping_label_in_a_row_keeps_its_longest_word_whole() {
    let _root = crate::reactive::RootScope::new();
    let text = NodeRef::new();
    let document = build_node(move || {
        view! {
            <List direction=Direction::Horizontal spacing=0.0>
                <Text @node_ref=&text @sizing=ItemSize::Percent(100.0)>{LABEL}</Text>
                <Frame width=180.0 height=20.0 />
            </List>
        }
    });
    let mut harness = Harness::sized(document, vec2(200.0, 100.0));
    harness.frame(Vec::new());
    let node = kind_of::<TextNode>(harness.document(), text.get());
    let layout = harness
        .document()
        .text_layout(node)
        .expect("the text was laid out");

    let starts: Vec<usize> = layout.lines.iter().map(|line| line.range.start).collect();
    assert_eq!(
        starts,
        vec![0, "Notifications ".len()],
        "the row gives the label room for its longest word, so it breaks only between words"
    );
    let rect = harness
        .document()
        .node_rect(text.get())
        .expect("the label was placed");
    let word = layout.lines[0].x_of("Notifications".len());
    assert!(
        rect.width() >= word,
        "the label is as wide as its longest word"
    );
    assert!(rect.width() < layout.size.x + 1.0);
}
