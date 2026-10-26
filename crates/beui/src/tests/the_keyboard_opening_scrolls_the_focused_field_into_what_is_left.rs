use super::*;
use crate::reactive::*;
use crate::styled::Scroll;
use crate::styled::*;
use crate::primitives::List;

#[test]
fn the_keyboard_opening_scrolls_the_focused_field_into_what_is_left() {
    let _root = crate::reactive::RootScope::new();
    let input = NodeRef::new();
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <Scroll @sizing=ItemSize::Percent(100.0)>
                    <List spacing=0.0>
                        <ForEach keys={indices(12)}>|index: usize|
                            <LabelledButton label={format!("Row {index}")} />
                        </ForEach>
                        <TextInput value="" @node_ref=&input />
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
    harness.resize(vec2(harness.size().x, 300.0));
    harness.frame(Vec::new());

    let field = harness.rect(input.get());
    assert!(
        field.top() >= 0.0 && field.bottom() <= 300.0,
        "the field sits at {field:?}, outside the 300 points the keyboard left"
    );
}
