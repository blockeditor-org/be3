use super::*;

#[test]
fn a_click_that_wakes_the_screens_reaches_nothing() {
    let mut gate = blanked();
    let left = Held::Button(0x110);

    assert_eq!(gate.pass(Input::Press(left), 0), swallowed(true));
    assert_eq!(
        gate.pass(Input::Release(left), 80 * MS),
        swallowed(false),
        "the release that pairs with the waking press goes with it"
    );

    assert_eq!(
        gate.pass(Input::Press(left), GRACE_USEC + 80 * MS),
        delivered(false),
        "once the grace period is over clicks reach the UI again"
    );
    assert_eq!(
        gate.pass(Input::Release(left), GRACE_USEC + 160 * MS),
        delivered(false)
    );

    gate.set_blanked(true);
    let key = Held::Key(30);
    assert_eq!(
        gate.pass(Input::Press(key), 10_000 * MS),
        swallowed(true),
        "a key wakes the screens the same way"
    );
    assert_eq!(
        gate.pass(Input::Release(key), 10_050 * MS),
        swallowed(false)
    );
}
