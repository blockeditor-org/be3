use super::*;

#[test]
fn a_held_modifier_is_lifted_for_a_tap_and_put_back_after_it() {
    let mut engine = engine(
        r#"
        [[group]]
        rules = [{ on = "Alt+W", not_held = ["Shift"], tap = "Shift+W" }, { on = "Alt+A", tap = "A", blind = true }]
        "#,
    );

    assert_eq!(engine.input(press("LeftAlt"), NOW), vec![press("LeftAlt")]);
    assert_eq!(
        engine.input(press("W"), NOW),
        vec![
            release("LeftAlt"),
            press("LeftShift"),
            press("W"),
            release("W"),
            release("LeftShift"),
            press("LeftAlt"),
        ]
    );
    assert_eq!(engine.input(release("W"), NOW), vec![]);
    assert_eq!(
        engine.input(press("A"), NOW),
        vec![press("A"), release("A")],
        "a blind tap leaves Alt held"
    );
}
