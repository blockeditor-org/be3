use super::*;
use crate::primitives::Frame;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn a_text_input_in_a_tall_slot_keeps_its_text_inside_its_field() {
    let _root = crate::reactive::RootScope::new();
    let input = NodeRef::new();
    let (document, [_slot]) = toolbar_of(move || {
        [view! {
            <Frame width=200.0 height=300.0>
                <TextInput @node_ref=&input value="hello" focused=true />
            </Frame>
        }]
    });
    let input = input.get();
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let output = harness.frame(Vec::new());

    let field = harness.rect(
        harness
            .document()
            .api::<unstyled::TextInputApi>(input)
            .text(),
    );
    let caret = output
        .main()
        .shapes()
        .iter()
        .find_map(|shape| match shape {
            crate::Shape::Rect {
                rect,
                stroke_width,
                color,
                ..
            } if *color == styled::Theme::DARK.accent && *stroke_width == 0.0 => Some(*rect),
            _ => None,
        })
        .expect("the focused input painted a caret");

    assert!(
        field.height() < 60.0,
        "the field is one line tall: {field:?}"
    );
    assert!(
        field.contains_rect(caret),
        "the caret {caret:?} is inside the field {field:?}"
    );
}
