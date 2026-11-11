use be_sdk::TimedInput;

use super::*;

#[test]
fn a_view_that_closes_with_the_keyboard_tells_the_program_and_one_without_does_not() {
    let (program, events) = watched();
    let mut surface = launched(&EditorHost::default(), program);
    let (main, side) = (whole(80.0, 60.0), preview());
    surface.frame(&[main.clone(), side.clone()]);
    assert_eq!(
        focus_events(&events),
        [false],
        "an app starts without the keyboard"
    );

    surface.input(&main, &TimedInput::from(Input::Focus(true)), None);
    surface.frame(&[main.clone(), side.clone()]);
    assert_eq!(focus_events(&events), [false, true]);

    // The view beside it never had the keyboard: closing it changes nothing.
    surface.frame(std::slice::from_ref(&main));
    assert_eq!(
        focus_events(&events),
        [false, true],
        "a view without the keyboard closes quietly"
    );

    surface.frame(std::slice::from_ref(&side));
    assert_eq!(
        focus_events(&events),
        [false, true, false],
        "the view that had the keyboard closed, so the app has none"
    );
}
