use super::*;

#[test]
fn choosing_never_keeps_the_screens_on() {
    let mut settings = Harness::in_a_session(vec![gaming_monitor()]);
    assert_eq!(settings.content().root().screen_off(), ScreenOff::DEFAULT);

    settings.choose(SCREEN_OFF, 7);
    settings
        .editor
        .snapshot("choosing_never_keeps_the_screens_on");

    assert_eq!(settings.content().root().screen_off(), ScreenOff::Never);
}
