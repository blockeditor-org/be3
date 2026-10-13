use super::*;

#[test]
fn a_key_held_across_the_wake_leaks_no_unmatched_release() {
    let mut gate = WakeGate::default();
    let shift = Held::Key(42);
    assert_eq!(gate.pass(Input::Press(shift), 0), delivered(false));

    gate.set_blanked(true);
    assert_eq!(
        gate.pass(Input::Release(shift), 600_000 * MS),
        delivered(true),
        "a key pressed before the screens went off is released where it was pressed"
    );

    gate.set_blanked(true);
    let finger = Held::Touch(0);
    let start = 900_000 * MS;
    assert_eq!(gate.pass(Input::Press(finger), start), swallowed(true));
    let after = start + GRACE_USEC * 3;
    assert_eq!(
        gate.pass(Input::Continue(finger), after),
        swallowed(false),
        "a finger that woke the screens stays swallowed while it moves"
    );
    assert_eq!(
        gate.pass(Input::Release(finger), after + MS),
        swallowed(false)
    );
    assert_eq!(
        gate.pass(Input::Release(finger), after + 2 * MS),
        delivered(false),
        "each swallowed press swallows one release"
    );
}
