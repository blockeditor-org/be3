use super::*;

const CTRL_C: &str = r#"
    [[group]]
    rules = [{ on = "Ctrl+C", send = "X" }]
    "#;

#[test]
fn keys_that_change_while_the_remaps_start_reach_them() {
    // Ctrl pressed before `Start` is answered.
    let mut remapper = Remaps::new(MODULE, Limits::default());
    remapper.configure(Some(CTRL_C.as_bytes().to_vec()));
    assert!(remapper.poll(NOW).is_empty());
    assert_eq!(
        remapper.input(press("LeftCtrl"), NOW),
        vec![press("LeftCtrl")]
    );
    remapper.started();
    assert!(
        remapper.input(press("C"), NOW).contains(&press("X")),
        "the remaps know Ctrl is held"
    );

    // Ctrl held as `Start` is sent and let go before it is answered.
    let mut remapper = Remaps::new(MODULE, Limits::default());
    assert_eq!(
        remapper.input(press("LeftCtrl"), NOW),
        vec![press("LeftCtrl")]
    );
    remapper.configure(Some(CTRL_C.as_bytes().to_vec()));
    assert!(remapper.poll(NOW).is_empty());
    remapper.input(release("LeftCtrl"), NOW);
    remapper.started();
    assert_eq!(
        remapper.input(press("C"), NOW),
        vec![press("C")],
        "the remaps know Ctrl was let go"
    );
    assert!(remapper.take_problems().is_empty());
}

#[test]
fn a_key_let_go_and_pressed_again_while_the_remaps_start_is_theirs() {
    let mut remapper = Remaps::new(MODULE, Limits::default());
    assert_eq!(
        remapper.input(press("LeftCtrl"), NOW),
        vec![press("LeftCtrl")]
    );
    remapper.configure(Some(CTRL_C.as_bytes().to_vec()));
    assert!(remapper.poll(NOW).is_empty(), "Start is told Ctrl is held");
    remapper.input(release("LeftCtrl"), NOW);
    remapper.input(press("LeftCtrl"), NOW);
    remapper.started();
    remapper.input(release("LeftCtrl"), NOW);
    assert_eq!(
        remapper.input(press("C"), NOW),
        vec![press("C")],
        "the remaps saw Ctrl let go"
    );
}

#[test]
fn a_key_that_bypassed_the_remaps_is_not_told_to_them_as_held_when_they_start_again() {
    let mut remapper = Remaps::new(MODULE, Limits::default());
    remapper.configure(Some(CTRL_C.as_bytes().to_vec()));
    assert!(remapper.poll(NOW).is_empty());
    assert_eq!(
        remapper.input(press("LeftCtrl"), NOW),
        vec![press("LeftCtrl")],
        "Ctrl pressed while they start bypasses them"
    );
    // Changed remaps start again, as after a failure, while Ctrl is held.
    remapper.configure(Some(format!("{CTRL_C}\n").into_bytes()));
    remapper.started();
    assert_eq!(
        remapper.input(release("LeftCtrl"), NOW),
        vec![release("LeftCtrl")]
    );
    assert_eq!(
        remapper.input(press("C"), NOW),
        vec![press("C")],
        "the remaps were never told Ctrl was held"
    );
}
