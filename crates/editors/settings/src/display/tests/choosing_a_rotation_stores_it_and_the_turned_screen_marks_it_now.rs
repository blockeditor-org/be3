use super::*;

#[test]
fn choosing_a_rotation_stores_it_and_the_turned_screen_marks_it_now() {
    let mut settings = Harness::in_a_session(vec![gaming_monitor()]);
    assert_eq!(settings.content().root().screen(MONITOR).rotation, None);

    settings.editor.click(tile(Rotation::Clockwise90));
    settings.run();

    let root = settings.content().root();
    assert_eq!(root.rotation(MONITOR), Rotation::Clockwise90);
    assert_eq!(root.mode(MONITOR), None, "the mode stays unset");

    settings
        .editor
        .set_watched::<DisplaysWatch>(&vec![turned(Rotation::Clockwise90)]);
    settings.run();
    settings
        .editor
        .snapshot("choosing_a_rotation_stores_it_and_the_turned_screen_marks_it_now");
    assert_eq!(
        settings
            .editor
            .accessible_label(&tile(Rotation::Clockwise90))
            .as_deref(),
        Some("Portrait")
    );

    settings.reset(ROTATION);
    assert_eq!(settings.content().root().screen(MONITOR).rotation, None);
}
