use super::*;

#[test]
fn super_backtick_switches_between_one_programs_windows() {
    let mut fixture = Fixture::new();
    fixture.settle();
    let kate = |id: u64, focused: bool| host_window(id, focused);
    fixture.show_windows(&[kate(1, false), foot(2), kate(3, false)]);
    fixture.show_windows(&[
        kate(1, false),
        TestWindow {
            focused: true,
            ..foot(2)
        },
        kate(3, false),
    ]);
    fixture.test.clear_calls();
    assert!(
        !fixture.test.host_key(Modifiers::LOGO, Key::Backtick),
        "a program with one window leaves Super+` alone"
    );

    fixture.show_windows(&[kate(1, false), foot(2), kate(3, true)]);
    fixture.test.clear_calls();
    assert!(fixture.test.host_key(Modifiers::LOGO, Key::Backtick));
    fixture.settle();
    assert_eq!(
        fixture.focused_windows(),
        [HostWindowId(1)],
        "Super+` moves to the program's next window, past the other program's"
    );
}
