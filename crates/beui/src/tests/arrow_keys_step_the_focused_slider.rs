use super::*;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn arrow_keys_step_the_focused_slider() {
    let (document, [slider]) = toolbar_of(|| {
        [view! {
            <Slider value=0.5 />
        }]
    });
    let mut harness = Harness::new(document);

    harness.key(Key::Tab, Modifiers::NONE);
    harness.key(Key::ArrowRight, Modifiers::NONE);

    let raised = harness
        .document()
        .api::<styled::SliderApi>(slider)
        .value()
        .get();
    assert!(raised > 0.5, "the slider read {raised}");

    harness.key(Key::ArrowLeft, Modifiers::NONE);
    harness.key(Key::ArrowLeft, Modifiers::NONE);

    let lowered = harness
        .document()
        .api::<styled::SliderApi>(slider)
        .value()
        .get();
    assert!(lowered < 0.5, "the slider read {lowered}");
}
