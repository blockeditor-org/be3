use super::*;

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
