use super::*;
use crate::reactive::{build, create_signal, view};
use crate::styled::Slider;

#[test]
fn a_driver_acts_on_a_slider_the_way_a_screen_reader_does() {
    let document = build(|| {
        let (value, set_value) = create_signal(0.5_f32);
        view! {
            <Slider
                value={value}
                label="Volume"
                on_change={move |next: f32| set_value.set(next)}
            />
        }
    });
    let mut driven = Driven::new(document);

    let offered = driven
        .ask(&["a11y", "Slider \"Volume\""])
        .expect("the slider's actions are listed");
    assert!(
        offered.contains("actions: ") && offered.contains("increment") && offered.contains("set"),
        "{offered}"
    );
    let changed = driven
        .ask(&["a11y", "Slider \"Volume\"", "set", "0.25"])
        .expect("the value is set");
    let added = |changed: &str, text: &str| {
        changed
            .lines()
            .any(|line| line.starts_with('+') && line.contains(text))
    };
    assert!(added(&changed, "Slider \"Volume\" value=\"0.25\""), "{changed}");
    let changed = driven
        .ask(&["a11y", "Slider \"Volume\"", "increment"])
        .expect("the value steps up");
    assert!(added(&changed, "Slider \"Volume\" value="), "{changed}");
    assert!(
        driven
            .ask(&["a11y", "Slider \"Volume\"", "expand"])
            .is_err(),
        "a slider does not expand"
    );
}
