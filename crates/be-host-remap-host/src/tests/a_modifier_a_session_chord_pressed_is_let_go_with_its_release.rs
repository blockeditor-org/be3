use super::*;

#[test]
fn a_modifier_a_session_chord_pressed_is_let_go_with_its_release() {
    let mut remapper = remapper(
        r#"
        [[group]]
        rules = [{ on = "LeftAlt", send = "Z" }]
        "#,
    );

    assert_eq!(
        remapper.input(press("LeftCtrl"), NOW),
        vec![press("LeftCtrl")]
    );
    assert_eq!(remapper.input(press("LeftAlt"), NOW), vec![press("Z")]);
    assert_eq!(
        remapper.input(press("F2"), NOW),
        vec![press("LeftAlt"), press("F2")]
    );
    assert_eq!(remapper.input(release("F2"), NOW), vec![release("F2")]);
    assert_eq!(
        remapper.input(release("LeftAlt"), NOW),
        vec![release("Z"), release("LeftAlt")],
        "the Alt the chord pressed is not left held"
    );
    assert_eq!(
        remapper.input(release("LeftCtrl"), NOW),
        vec![release("LeftCtrl")]
    );
}
