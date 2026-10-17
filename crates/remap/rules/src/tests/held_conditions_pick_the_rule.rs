use super::*;

#[test]
fn held_conditions_pick_the_rule() {
    let mut engine = engine(
        r#"
        [[group]]
        rules = [{ on = "Alt+W", not_held = ["Shift"], tap = "Shift+W" }]
        "#,
    );

    assert_eq!(engine.input(press("W"), NOW), vec![press("W")]);
    assert_eq!(engine.input(release("W"), NOW), vec![release("W")]);
    engine.input(press("RightShift"), NOW);
    engine.input(press("LeftAlt"), NOW);
    assert_eq!(
        engine.input(press("W"), NOW),
        vec![press("W")],
        "Shift held keeps the rule from applying"
    );
}
