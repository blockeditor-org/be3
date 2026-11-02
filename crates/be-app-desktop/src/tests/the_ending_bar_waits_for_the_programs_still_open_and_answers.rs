use be_sdk_beui::beui::{Pos2, Rect, Vec2};

use super::*;

#[test]
fn the_ending_bar_waits_for_the_programs_still_open_and_answers() {
    let mut fixture = Fixture::new();
    fixture.settle();
    fixture.allow_power(EVERYTHING);
    fixture.show_windows(&[foot(1), host_window(2, true)]);
    fixture.test.show_on_displays(&[(
        Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0)),
        1.0,
    )]);
    fixture.settle();
    assert!(!fixture.test.shown("desktop.ending"));

    fixture.allow_power(ending(SessionEnd::LogOut));
    fixture.show_windows(&[host_window(2, true)]);
    assert!(fixture.test.shown("desktop.ending"));
    assert!(fixture.says("Logging out"));
    assert!(
        fixture.says("Waiting for Kate to close."),
        "the program asking about unsaved changes is named"
    );
    assert!(fixture.says("Log out now"));
    fixture
        .test
        .desk_snapshot("logging_out_waits_for_a_program_still_open", 0);

    fixture.test.click("desktop.ending.now");
    fixture.settle();
    fixture.test.click("desktop.ending.cancel");
    fixture.settle();
    assert_eq!(
        fixture
            .test
            .take_host_calls(be_sdk_test::requested::ending_answer),
        vec![EndingAnswer::EndNow, EndingAnswer::Cancel]
    );

    fixture.allow_power(ending(SessionEnd::PowerOff));
    fixture.show_windows(&[
        foot(3),
        notes(4),
        host_window(2, true),
        host_window(5, false),
    ]);
    assert!(fixture.says("Powering off"));
    assert!(fixture.says("Power off now"));
    assert!(
        fixture.says("Waiting for Foot, Shopping list and Kate to close."),
        "each program is named once, by its name or else its window's title"
    );

    fixture.allow_power(EVERYTHING);
    assert!(
        !fixture.test.shown("desktop.ending"),
        "the bar goes once the session goes on"
    );
}
