use super::*;

#[test]
fn choosing_never_keeps_the_screens_on() {
    let mut editor = editor(vec![gaming_monitor()]);
    assert_eq!(content(&editor).root().screen_off(), ScreenOff::DEFAULT);

    choose(&mut editor, SCREEN_OFF, 7);
    editor.snapshot("choosing_never_keeps_the_screens_on");

    assert_eq!(content(&editor).root().screen_off(), ScreenOff::Never);
}
