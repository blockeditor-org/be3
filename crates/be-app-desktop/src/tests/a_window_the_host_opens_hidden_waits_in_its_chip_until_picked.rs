use super::*;

#[test]
fn a_window_the_host_opens_hidden_waits_in_its_chip_until_picked() {
    let mut fixture = Fixture::new();
    fixture.settle();
    let captured = |window: TestWindow| TestWindow {
        opens_hidden: true,
        ..window
    };
    fixture.show_windows(&[captured(foot(1)), captured(notes(2))]);
    fixture.settle();
    assert!(
        fixture.window_rect(1).is_none() && fixture.window_rect(2).is_none(),
        "a window the host opens hidden (each Windows window, which showing captures) is placed nowhere"
    );
    assert!(
        fixture.test.shown("desktop.windows.1") && fixture.test.shown("desktop.windows.2"),
        "but has its chip, to show it from"
    );
    assert!(
        fixture.focused_windows().is_empty(),
        "and nothing is given the keyboard"
    );

    fixture.test.click("desktop.windows.1");
    fixture.settle();
    assert!(fixture.window_rect(1).is_some(), "its chip shows it");
    assert!(
        fixture.window_rect(2).is_none(),
        "and only it: the other still waits"
    );
    assert_eq!(
        fixture.focused_windows(),
        [HostWindowId(1)],
        "the window picked is given the keyboard"
    );

    fixture.show_windows(&[captured(foot(1)), captured(notes(2)), foot(3)]);
    fixture.settle();
    assert!(
        fixture.window_rect(3).is_some(),
        "a window the host does not open hidden (a Wayland client) shows at once"
    );
}
