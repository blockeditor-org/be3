use super::*;

#[test]
fn a_key_held_before_the_remaps_saw_it_is_released_as_it_is() {
    let mut remapper = remapper(
        r#"
        [[group]]
        rules = [{ on = "LeftCtrl", send = "Z" }]
        "#,
    );

    assert_eq!(
        remapper.input(release("LeftCtrl"), NOW),
        vec![release("LeftCtrl")],
        "a Ctrl pressed before these remaps were installed is let go"
    );
    assert_eq!(remapper.input(press("LeftCtrl"), NOW), vec![press("Z")]);
    remapper.forget_held();
    assert_eq!(
        remapper.input(release("LeftCtrl"), NOW),
        vec![release("LeftCtrl")]
    );
}
