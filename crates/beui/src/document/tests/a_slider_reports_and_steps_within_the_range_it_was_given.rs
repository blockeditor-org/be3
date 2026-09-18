use super::*;
use crate::reactive::view;
use crate::styled::Slider;

#[test]
fn a_slider_reports_and_steps_within_the_range_it_was_given() {
    let reported = Rc::new(RefCell::new(Vec::new()));
    let sink = reported.clone();
    let (document, [slider]) = toolbar_of(|| {
        [view! {
            <Slider
                value=1.0
                min=0.5
                max=3.0
                label="Zoom"
                on_change={move |value| sink.borrow_mut().push(value)}
            />
        }]
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    harness.key(Key::Tab, Modifiers::NONE);
    harness.key(Key::ArrowRight, Modifiers::NONE);
    let stepped = styled::slider_value(harness.document(), slider);
    assert!((stepped - 1.125).abs() < 0.001, "the slider read {stepped}");

    harness.key(Key::End, Modifiers::NONE);
    assert_eq!(styled::slider_value(harness.document(), slider), 3.0);

    harness.key(Key::Home, Modifiers::NONE);
    assert_eq!(styled::slider_value(harness.document(), slider), 0.5);

    assert_eq!(*reported.borrow(), [1.125, 3.0, 0.5]);
}
