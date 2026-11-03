use super::*;

#[test]
fn choosing_a_display_scale_stores_it_and_the_default_suits_the_display() {
    let laptop = HostDisplay {
        scale: 1.75,
        default_scale: 1.75,
        ..gaming_monitor()
    };
    let mut settings = Harness::in_a_session(vec![laptop]);
    assert!(
        settings.says("Default (175%)"),
        "until one is chosen, the display's own default is offered"
    );
    settings
        .editor
        .snapshot("choosing_a_display_scale_stores_it_and_the_default_suits_the_display");

    settings.choose(DISPLAY_SCALE, 5);
    let root = settings.content().root();
    assert_eq!(root.screen(MONITOR).scale_percent, Some(200));
    assert_eq!(root.screen_scale(MONITOR), Some(2.0));
    assert_eq!(root.mode(MONITOR), None, "the mode stays unset");

    settings.reset(DISPLAY_SCALE);
    assert_eq!(
        settings.content().root().screen(MONITOR).scale_percent,
        None
    );
    assert_eq!(settings.content().root().screen_scale(MONITOR), None);
}
