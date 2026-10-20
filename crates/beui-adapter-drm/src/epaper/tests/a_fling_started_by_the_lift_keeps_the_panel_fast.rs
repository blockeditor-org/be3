use super::*;

#[test]
fn a_fling_started_by_the_lift_keeps_the_panel_fast() {
    let at = Duration::from_millis;
    let finger = Held::Touch(0);
    let mut pace = Pace::default();
    pace.input(Input::Press(finger), at(0));
    assert_eq!(pace.step(at(0)), Some(Change::Hurry));
    pace.input(Input::Release(finger), at(200));
    assert_eq!(pace.step(at(200)), None);
    pace.animating(true);
    pace.drew(at(210));
    assert_eq!(
        pace.step(at(900)),
        None,
        "the fling is drawn fast without settling first"
    );
    pace.animating(false);
    pace.drew(at(900));
    assert_eq!(pace.step(at(900) + QUIET), Some(Change::Settle));
}
