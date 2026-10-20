use super::*;

#[test]
fn input_hurries_the_panel_until_it_has_been_still_for_a_moment() {
    let at = Duration::from_millis;
    let mut pace = Pace::default();
    assert_eq!(pace.step(at(0)), None);
    pace.touched(at(100));
    assert_eq!(pace.step(at(100)), Some(Change::Hurry));
    assert_eq!(pace.settles_at(), Some(at(100) + SETTLE_AFTER_INPUT));
    pace.touched(at(300));
    assert_eq!(pace.step(at(500)), None);
    assert_eq!(
        pace.step(at(300) + SETTLE_AFTER_INPUT),
        Some(Change::Settle)
    );
    assert_eq!(pace.settles_at(), None);
}
