use super::*;

#[test]
fn input_in_the_grace_period_is_swallowed_and_input_after_it_is_delivered() {
    let mut gate = blanked();

    assert_eq!(
        gate.pass(Input::Other, 0),
        swallowed(true),
        "moving the mouse wakes the screens"
    );
    assert_eq!(gate.pass(Input::Other, 100 * MS), swallowed(false));
    assert_eq!(
        gate.pass(Input::Press(Held::Key(30)), GRACE_USEC - MS),
        swallowed(false),
        "a key pressed while the monitor comes back on lands nowhere"
    );

    assert_eq!(gate.pass(Input::Other, GRACE_USEC), delivered(false));
    assert_eq!(
        gate.pass(Input::Press(Held::Key(31)), GRACE_USEC + MS),
        delivered(false)
    );
    assert_eq!(
        gate.pass(Input::Release(Held::Key(30)), GRACE_USEC + 2 * MS),
        swallowed(false),
        "the release of a swallowed press is swallowed after the grace period too"
    );
    assert_eq!(
        gate.pass(Input::Release(Held::Key(31)), GRACE_USEC + 3 * MS),
        delivered(false)
    );
}
