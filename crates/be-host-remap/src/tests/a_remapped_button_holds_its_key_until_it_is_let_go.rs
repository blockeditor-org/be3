use super::*;

#[test]
fn a_remapped_button_holds_its_key_until_it_is_let_go() {
    let mut engine = engine(
        r#"
        [[group]]
        rules = [{ on = "LButton", send = "X" }]
        "#,
    );

    assert_eq!(engine.input(press("MouseLeft"), NOW), vec![press("X")]);
    assert_eq!(engine.input(press("A"), NOW), vec![press("A")]);
    assert_eq!(engine.input(release("MouseLeft"), NOW), vec![release("X")]);
    assert_eq!(engine.input(release("A"), NOW), vec![release("A")]);
    assert_eq!(engine.deadline(), None);
}
