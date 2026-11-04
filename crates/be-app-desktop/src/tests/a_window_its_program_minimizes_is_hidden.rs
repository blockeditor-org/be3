use super::*;

#[test]
fn a_window_its_program_minimizes_is_hidden() {
    let mut fixture = Fixture::with_windows(&[1, 2]);
    fixture.show_windows(&[
        TestWindow {
            minimized: 1,
            ..host_window(1, false)
        },
        host_window(2, true),
    ]);
    assert!(
        fixture.window_rect(1).is_none(),
        "a window its program minimizes is hidden"
    );
    assert!(fixture.window_rect(2).is_some(), "and only that one");
    assert!(
        fixture.test.shown("desktop.windows.1"),
        "it keeps its chip to show it again"
    );

    fixture.test.click("desktop.windows.1");
    fixture.settle();
    assert!(
        fixture.window_rect(1).is_some(),
        "the chip shows it again, and it stays shown until its program minimizes it again"
    );
    fixture.show_windows(&[
        TestWindow {
            minimized: 2,
            ..host_window(1, false)
        },
        host_window(2, true),
    ]);
    assert!(fixture.window_rect(1).is_none());
}
