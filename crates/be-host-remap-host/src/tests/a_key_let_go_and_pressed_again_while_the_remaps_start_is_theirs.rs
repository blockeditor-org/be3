use super::*;

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
