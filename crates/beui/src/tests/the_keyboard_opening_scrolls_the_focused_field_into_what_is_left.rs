use super::*;
use crate::reactive::{ForEach, ItemSize, List, NodeRef, build, view};
use crate::styled::{Scroll, TextInput};

#[test]
fn the_keyboard_opening_scrolls_the_focused_field_into_what_is_left() {
    let input = NodeRef::new();
    let input_ref = input.clone();
    let document = build(move || {
        view! {
            <List spacing=0.0>
                <Scroll @sizing=ItemSize::Percent(100.0)>
                    <List spacing=0.0>
                        <ForEach keys={indices(12)}>
                            {|index: usize| view! {
                                <LabelledButton label={format!("Row {index}")} />
                            }}
                        </ForEach>
                        <TextInput value="" @node_ref=&input_ref />
                    </List>
                </Scroll>
            </List>
        }
    });
    let mut harness = Harness::sized(document, Vec2::new(400.0, 600.0));
    harness.frame(Vec::new());
    let field = harness.rect(input.get());
    assert!(field.bottom() <= 600.0 && field.top() > 300.0);

    harness.click(field.center());
    harness.viewport_mut().y = 300.0;
    harness.frame(Vec::new());

    let field = harness.rect(input.get());
    assert!(
        field.top() >= 0.0 && field.bottom() <= 300.0,
        "the field sits at {field:?}, outside the 300 points the keyboard left"
    );
}
