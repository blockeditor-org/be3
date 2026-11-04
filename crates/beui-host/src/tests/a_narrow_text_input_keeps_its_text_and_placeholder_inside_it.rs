use super::*;
use crate::painter::Shape;
use crate::reactive::*;
use crate::styled::{self, Size, TextInput};
use beui::Direction;

fn texts(output: &crate::native::FrameOutput) -> Vec<(Pos2, Rect)> {
    output
        .main()
        .shapes()
        .into_iter()
        .filter_map(|shape| match shape {
            Shape::Text { origin, clip, .. } => Some((origin, clip)),
            _ => None,
        })
        .collect()
}

#[test]
fn a_narrow_text_input_keeps_its_text_and_placeholder_inside_it() {
    let _root = crate::reactive::RootScope::new();
    let value = create_signal(String::new());
    let field = NodeRef::new();
    let (document, _) = toolbar_of(move || {
        [view! {
            <styled::List direction=Direction::Horizontal spacing=styled::Space::None>
                <styled::Frame width={Size::Custom(90.0)}>
                    <TextInput
                        @node_ref=&field
                        value={value}
                        placeholder="A placeholder much longer than the field"
                    />
                </styled::Frame>
            </styled::List>
        }]
    });
    let mut harness = Harness::new(document);
    let output = harness.frame(Vec::new());
    let field = harness.rect(field.get());
    let inside = |output: &crate::native::FrameOutput, what: &str| {
        for (_, clip) in texts(output) {
            assert!(
                clip.max.x <= field.max.x + 0.5,
                "{what} is clipped to the field {field:?}, not {clip:?}"
            );
        }
    };
    inside(&output, "the placeholder");

    harness.click(field.center());
    harness.frame(vec![Input::Text(
        "a long line typed into the field".to_owned(),
    )]);
    let output = harness.frame(Vec::new());
    inside(&output, "typed text scrolled along");

    value.set(String::new());
    harness.frame(Vec::new());
    let output = harness.frame(Vec::new());
    let (origin, _) = texts(&output)
        .into_iter()
        .min_by(|a, b| a.0.x.total_cmp(&b.0.x))
        .expect("the placeholder is painted");
    assert!(
        origin.x >= field.min.x,
        "emptied, the field shows its placeholder from its start, not scrolled: {origin:?} in {field:?}"
    );
}
