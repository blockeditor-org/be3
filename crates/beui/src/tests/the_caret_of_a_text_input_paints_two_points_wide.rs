use super::*;
use crate::reactive::view;
use crate::styled::TextInput;

const CARET_WIDTH: f32 = 2.0;

#[test]
fn the_caret_of_a_text_input_paints_two_points_wide() {
    let (document, [input]) = toolbar_of(|| {
        [view! {
            <TextInput value="Text" />
        }]
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    with_installed(harness.document_mut(), |_| {
        crate::focus_within(input);
    });
    let output = harness.frame(Vec::new());

    let caret = output
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

    assert_eq!(caret.width(), CARET_WIDTH);
}
