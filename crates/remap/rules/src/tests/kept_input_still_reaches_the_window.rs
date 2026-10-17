use super::*;

#[test]
fn kept_input_still_reaches_the_window() {
    let mut engine = engine(
        r#"
        [[group]]
        rules = [{ on = "MouseLeft", send = "F1", keep = true }]
        "#,
    );

    assert_eq!(
        engine.input(press("MouseLeft"), NOW),
        vec![press("MouseLeft"), press("F1")]
    );
    assert_eq!(
        engine.input(release("MouseLeft"), NOW),
        vec![release("F1"), release("MouseLeft")]
    );
}
