use super::*;

#[test]
fn wheel_turns_no_rule_takes_pass_through() {
    let mut engine = engine(
        r#"
        [[group]]
        rules = [{ on = "F20+WheelUp", send = "VolumeUp" }]
        "#,
    );

    assert_eq!(
        engine.input(Input::Wheel { x: 30, y: -120 }, NOW),
        vec![Input::Wheel { x: 30, y: -120 }],
        "without F20 the wheel scrolls"
    );
    assert_eq!(engine.input(press("F20"), NOW), vec![press("F20")]);
    assert_eq!(
        engine.input(Input::Wheel { x: 30, y: -120 }, NOW),
        vec![
            Input::Wheel { x: 30, y: 0 },
            press("VolumeUp"),
            release("VolumeUp")
        ],
        "only the axis a rule takes stops scrolling"
    );
}
