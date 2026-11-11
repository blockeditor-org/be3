use super::*;
use be_sdk::TimedInput;

#[test]
fn a_view_given_the_keyboard_that_never_shows_does_not_hold_it_for_ever() {
    let (program, events) = watched();
    let mut surface = launched(&EditorHost::default(), program);
    let (main, side) = (whole(80.0, 60.0), preview());
    surface.frame(std::slice::from_ref(&main));
    surface.input(&side, &TimedInput::from(Input::Focus(true)), None);
    surface.frame(std::slice::from_ref(&main));
    surface.frame(std::slice::from_ref(&main));
    assert_eq!(
        focus_events(&events),
        [false, true, false],
        "a view closed before its first frame gives the keyboard back"
    );
}
