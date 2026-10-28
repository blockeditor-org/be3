use super::*;
use crate::reactive::*;
use crate::styled::*;

const RING_ALLOWANCE: f32 = 10.0;

#[test]
fn the_focus_ring_of_a_select_hugs_its_trigger_not_the_row_beside_it() {
    let (document, [select]) = toolbar_of(|| {
        let options = view! {
            <unstyled::ChoiceOption value="Apple" label="Apple" />
            <unstyled::ChoiceOption value="Banana" label="Banana" />
            <unstyled::ChoiceOption value="Cherry" label="Cherry" />
        };
        [view! {
            <Select options selected=None />
        }]
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    harness.key(Key::Tab, Modifiers::NONE);
    let trigger = harness
        .document()
        .api::<unstyled::SelectApi<&str>>(select)
        .trigger();
    assert!(harness.document().focus_is_within(trigger));
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

fn painted(output: &crate::native::FrameOutput, fill: Color32, stroke: f32) -> Option<Rect> {
    output.main().shapes().iter().find_map(|shape| match shape {
        crate::native::Shape::Rect {
            rect,
            stroke_width,
            color,
            ..
        } if *color == fill && *stroke_width == stroke => Some(*rect),
        _ => None,
    })
}
