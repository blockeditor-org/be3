use super::*;

#[test]
fn outside_a_session_only_the_appearance_is_shown() {
    let settings = Harness::new();
    assert!(settings.editor.shown(THEME));
    assert!(settings.editor.shown(SCALE));
    assert!(
        !settings.editor.shown(SCREEN_OFF),
        "only a session has power settings"
    );
    assert!(
        !settings.says("Screens"),
        "only a session reports its screens"
    );

    let settings = Harness::in_a_session(Vec::new());
    assert!(settings.editor.shown(SCREEN_OFF));
    assert!(settings.says("No screen is connected to this session."));
}
