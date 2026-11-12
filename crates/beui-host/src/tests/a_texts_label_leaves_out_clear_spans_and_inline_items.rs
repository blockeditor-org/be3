use accesskit::Role;

use super::*;
use crate::color::Color32;
use crate::primitives::Span;
use crate::primitives::*;

#[test]
fn a_texts_label_leaves_out_clear_spans_and_inline_items() {
    let _root = crate::reactive::RootScope::new();
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <Text color=Color32::WHITE>
                    <TextItem>
                        <Frame width=10.0 height=10.0 />
                    </TextItem>
                    <Span>"item a1"</Span>
                    <Span color={Some(Color32::TRANSPARENT.into())}>"mm"</Span>
                </Text>
            </List>
        }
    });
    let mut harness = Harness::new(document);

    let output = harness.frame(Vec::new());
    let tree = output.accessibility_tree("Test", VIEWPORT);
    let labels: Vec<_> = tree
        .nodes
        .iter()
        .filter(|(_, node)| node.role() == Role::Label)
        .filter_map(|(_, node)| node.value())
        .collect();
    assert_eq!(
        labels,
        vec!["item a1"],
        "a screen reader reads the text shown: not a slot drawn in a clear color, such as a text \
         area's newline, nor the object character standing in for an inline item"
    );
}
