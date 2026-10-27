use super::the_chrome_around_a_curved_panel::window;
use super::*;

#[test]
fn a_window_s_notice_shows_in_place_of_its_title() {
    let (mut space, now, context) = painted_space();
    let popped = open(
        &mut space,
        now,
        WindowPlacement::Over {
            window: WindowId::MAIN,
            at: pos2(120.0, 240.0),
        },
    )
    .0;
    let mut main = window(&context, WindowId::MAIN, PLATFORM, "Workspace");
    main.window.appearance.title = Some("Workspace".to_owned());
    let mut noted = window(&context, popped, SMALL, "Paint");
    noted.window.appearance.title = Some("Paint".to_owned());
    space.compose(&[main.clone(), noted.clone()], &[], &view(now));
    let titled = space.panel(popped).unwrap().chrome.title;

    let notice = "Runs as administrator, so Block can't control it";
    noted.window.appearance.notice = Some(notice.to_owned());
    let windows = [main, noted];
    space.compose(&windows, &[], &view(now));
    send(&mut space, now, []);
    let measured = space.panel(popped).unwrap().chrome.title;
    assert!(
        measured > titled,
        "the notice is measured in place of the shorter title: {titled} then {measured}"
    );
    assert_scene(
        &mut space,
        now,
        &windows,
        "a_window_s_notice_shows_in_place_of_its_title",
    );
}
