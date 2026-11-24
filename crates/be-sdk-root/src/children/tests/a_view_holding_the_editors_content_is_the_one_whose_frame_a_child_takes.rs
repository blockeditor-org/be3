use super::*;

#[test]
fn a_view_holding_the_editors_content_is_the_one_whose_frame_a_child_takes() {
    let mut main = child_screen(
        &placed(rect((0.0, 0.0), (300.0, 200.0)), ChildMode::Live),
        EditorInstanceId(1),
        1.0,
        1.0,
    );
    main.screen = ScreenId(1);
    let mut bar = main.clone();
    bar.screen = ScreenId(2);
    bar.view = ViewId(3);
    bar.placement = ViewPlacement::Dock;
    let mut tab = bar.clone();
    tab.screen = ScreenId(3);
    tab.view = ViewId(4);
    let screens = [main, bar, tab];
    let view = |holding: &[u64]| {
        let holds = |screen: ScreenId| holding.contains(&screen.0);
        content_view(EditorInstanceId(1), &screens, &holds)
    };
    // Its main view while that holds the content; once docked, the tab its content moved to,
    // not a bar of its tools; its main view until any says.
    assert_eq!(view(&[1, 3]), ViewId::MAIN);
    assert_eq!(view(&[3]), ViewId(4));
    assert_eq!(view(&[]), ViewId::MAIN);
}
