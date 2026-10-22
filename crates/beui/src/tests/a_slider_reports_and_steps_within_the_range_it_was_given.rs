use super::*;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn a_slider_reports_and_steps_within_the_range_it_was_given() {
    let _root = crate::reactive::RootScope::new();
    let reported = create_signal(Vec::new());
    let (document, [slider]) = toolbar_of(|| {
        [view! {
            <Slider
                value={Binding::new(1.0, move |next| reported.update(|changes| changes.push(next)))}
                min=0.5
                max=3.0
                label="Zoom"
            />
        }]
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    harness.key(Key::Tab, Modifiers::NONE);
    harness.key(Key::ArrowRight, Modifiers::NONE);
    let stepped = harness
        .document()
        .api::<styled::SliderApi>(slider)
        .value()
        .get();
    assert!((stepped - 1.125).abs() < 0.001, "the slider read {stepped}");

    harness.key(Key::End, Modifiers::NONE);
    assert_eq!(
        harness
            .document()
            .api::<styled::SliderApi>(slider)
            .value()
            .get(),
        3.0
    );

    harness.key(Key::Home, Modifiers::NONE);
    assert_eq!(
        harness
            .document()
            .api::<styled::SliderApi>(slider)
            .value()
            .get(),
        0.5
    );

    assert_eq!(reported.get_untracked(), [1.125, 3.0, 0.5]);
}
