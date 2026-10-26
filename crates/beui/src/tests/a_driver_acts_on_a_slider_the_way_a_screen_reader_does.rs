use super::*;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn a_driver_acts_on_a_slider_the_way_a_screen_reader_does() {
    let document = build_node(|| {
        let value = create_signal(0.5_f32);
        view! {
            <Slider value={value} label="Volume" />
        }
    });
    let mut driven = stepped(crate::testing::Showing(document));

    let offered = driven
        .ask(&["app", "a11y", "Slider \"Volume\""])
        .expect("the slider's actions are listed");
    assert!(
        offered.contains("actions: ") && offered.contains("increment") && offered.contains("set"),
        "{offered}"
    );
    let changed = driven
        .ask(&["app", "a11y", "Slider \"Volume\"", "set", "0.25"])
        .expect("the value is set");
    let added = |changed: &str, text: &str| {
        changed
            .lines()
            .any(|line| line.starts_with('+') && line.contains(text))
    };
    assert!(
        added(&changed, "Slider \"Volume\" value=\"0.25\""),
        "{changed}"
    );
    let changed = driven
        .ask(&["app", "a11y", "Slider \"Volume\"", "increment"])
        .expect("the value steps up");
    assert!(added(&changed, "Slider \"Volume\" value="), "{changed}");
    assert!(
        driven
            .ask(&["app", "a11y", "Slider \"Volume\"", "expand"])
            .is_err(),
        "a slider does not expand"
    );
}
