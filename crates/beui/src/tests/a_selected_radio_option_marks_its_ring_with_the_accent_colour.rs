use super::*;
use crate::reactive::view;
use crate::styled::RadioGroup;

const MARK_BOX: f32 = 18.0;
const RING_WIDTH: f32 = 2.0;

#[test]
fn a_selected_radio_option_marks_its_ring_with_the_accent_colour() {
    assert_eq!(accent_rings(None), 0);
    assert_eq!(accent_rings(Some(0)), 1);
}

fn accent_rings(selected: Option<usize>) -> usize {
    let (document, [_group]) = toolbar_of(move || {
        [view! {
            <RadioGroup
                options={view! {
                    <unstyled::ChoiceOption label="One" />
                    <unstyled::ChoiceOption label="Two" />
                }}
                selected
            />
        }]
    });
    let mut harness = Harness::new(document);
    let output = harness.frame(Vec::new());
    output
        .shapes()
        .iter()
        .filter(|shape| {
            matches!(shape, crate::Shape::Rect { rect, stroke_width, color, .. }
                if *color == styled::Theme::DARK.accent
                    && *stroke_width == RING_WIDTH
                    && rect.width() == MARK_BOX)
        })
        .count()
}
