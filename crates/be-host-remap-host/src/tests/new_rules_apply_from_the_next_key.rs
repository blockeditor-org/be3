use super::*;

#[test]
fn new_rules_apply_from_the_next_key() {
    let mut remapper = remapper(
        r#"
        [[group]]
        name = "Everywhere"
        rules = []
        "#,
    );
    let terminal = Window {
        exe: "foot".to_owned(),
        ..Window::default()
    };
    assert_eq!(remapper.focus(Some(terminal), NOW), vec![]);
    assert_eq!(remapper.input(press("CapsLock"), NOW), vec![press("CapsLock")]);
    assert_eq!(
        remapper.input(release("CapsLock"), NOW),
        vec![release("CapsLock")]
    );

    let half_written = br#"
        [[group]]
        name = "Everywhere"
        rules = [{ on = "CapsLock" }]
        "#;
    assert_eq!(remapper.configure(Some(half_written.to_vec())), vec![]);
    remapper.input(press("CapsLock"), NOW);
    remapper.input(release("CapsLock"), NOW);
    remapper.take_problems();

    let written = br#"
        [[group]]
        name = "Everywhere"
        rules = [{ on = "CapsLock", send = "Delete" }]
        "#;
    assert_eq!(remapper.configure(Some(written.to_vec())), vec![]);
    assert_eq!(remapper.input(press("CapsLock"), NOW), vec![press("Delete")]);
    assert_eq!(
        remapper.input(release("CapsLock"), NOW),
        vec![release("Delete")]
    );
    assert!(remapper.take_problems().is_empty());
}
