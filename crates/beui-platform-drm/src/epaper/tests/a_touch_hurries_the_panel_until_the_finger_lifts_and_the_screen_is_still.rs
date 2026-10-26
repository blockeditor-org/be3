use super::*;

#[test]
fn a_touch_hurries_the_panel_until_the_finger_lifts_and_the_screen_is_still() {
    let at = Duration::from_millis;
    let finger = Held::Touch(0);
    let mut pace = Pace::default();
    pace.input(Input::Press(finger), at(0));
    assert_eq!(pace.step(at(0)), Some(Change::Hurry));
    pace.drew(at(400));
    assert_eq!(
        pace.step(at(1_000)),
        None,
        "a finger on the screen keeps it fast"
    );
    assert_eq!(pace.settles_at(), None);

    pace.input(Input::Release(finger), at(1_000));
    assert_eq!(pace.step(at(1_000)), None, "the lift is drawn fast too");
    pace.drew(at(1_010));
    assert_eq!(pace.settles_at(), Some(at(1_010) + QUIET));
    assert_eq!(pace.step(at(1_050)), None);
    assert_eq!(pace.step(at(1_010) + QUIET), Some(Change::Settle));
}
