use super::*;
use crate::primitives::Frame;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn typing_past_the_end_of_a_narrow_text_input_scrolls_the_caret_into_view() {
    let _root = crate::reactive::RootScope::new();
    let value = "a value that is much wider than the field";
    let input = NodeRef::new();
    let (document, [_sized]) = toolbar_of(move || {
        [view! {
            <Frame width=80.0>
                <TextInput @node_ref=&input value=String::new() />
            </Frame>
        }]
    });
    let input = input.get();
    let mut harness = Harness::new(document);

    harness.key(Key::Tab, Modifiers::NONE);
    harness.type_text(value);
    harness.frame(Vec::new());

    let text = harness
        .document()
        .api::<unstyled::TextInputApi>(input)
        .text();
    let rect = harness.rect(text);
    let caret = harness
        .document()
        .api::<unstyled::TextInputApi>(input)
        .index_at(
            harness.document(),
            pos2(rect.right() - 1.0, rect.center().y),
        );

    assert_eq!(caret, value.len());
}
