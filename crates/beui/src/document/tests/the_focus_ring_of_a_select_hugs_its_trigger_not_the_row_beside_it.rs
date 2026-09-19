use super::*;
use crate::reactive::view;
use crate::styled::Select;

const RING_ALLOWANCE: f32 = 10.0;

#[test]
fn the_focus_ring_of_a_select_hugs_its_trigger_not_the_row_beside_it() {
    let (document, [select]) = toolbar_of(|| {
        let options = view! {
            <unstyled::ChoiceOption label="Apple" />
            <unstyled::ChoiceOption label="Banana" />
            <unstyled::ChoiceOption label="Cherry" />
        };
        [view! {
            <Select options selected=None />
        }]
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let trigger = unstyled::select_trigger(harness.document(), select);
    with_installed(harness.document_mut(), |_| {
        crate::focus_within(trigger);
    });
    let output = harness.frame(Vec::new());

    let face = painted(&output, styled::Theme::DARK.surface_raised, 0.0)
        .expect("the trigger painted its face");
    let ring = painted(&output, styled::Theme::DARK.accent, 2.0)
        .expect("the trigger painted a focus ring");
    assert!(
        ring.width() - face.width() < RING_ALLOWANCE,
        "the ring is {} wide around a {} wide trigger",
        ring.width(),
        face.width()
    );
}

fn painted(output: &crate::FrameOutput, fill: Color32, stroke: f32) -> Option<Rect> {
    output.shapes().iter().find_map(|shape| match shape {
        crate::Shape::Rect {
            rect,
            stroke_width,
            color,
            ..
        } if *color == fill && *stroke_width == stroke => Some(*rect),
        _ => None,
    })
}
