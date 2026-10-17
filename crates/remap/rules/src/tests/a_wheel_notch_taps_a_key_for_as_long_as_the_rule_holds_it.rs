use super::*;

#[test]
fn a_wheel_notch_taps_a_key_for_as_long_as_the_rule_holds_it() {
    let mut engine = engine(
        r#"
        [[group]]
        rules = [{ on = "WheelUp", tap = "1", hold_ms = 20 }, { on = "WheelDown", send = "3" }]
        "#,
    );

    assert_eq!(
        engine.input(Input::Wheel { x: 0, y: -120 }, NOW),
        vec![press("1")]
    );
    assert_eq!(
        engine.input(Input::Wheel { x: 0, y: -120 }, NOW + 5 * MS),
        vec![],
        "a second notch waits for the first tap to finish"
    );
    assert_eq!(engine.timer(NOW + 20 * MS), vec![release("1"), press("1")]);
    assert_eq!(engine.timer(NOW + 40 * MS), vec![release("1")]);

    assert_eq!(
        engine.input(Input::Wheel { x: 0, y: 60 }, NOW + 50 * MS),
        vec![],
        "half a notch of a high-resolution wheel is not a notch yet"
    );
    assert_eq!(
        engine.input(Input::Wheel { x: 0, y: 60 }, NOW + 51 * MS),
        vec![press("3"), release("3")]
    );
}
