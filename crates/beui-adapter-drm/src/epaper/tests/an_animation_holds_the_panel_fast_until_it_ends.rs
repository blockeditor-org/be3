use super::*;

#[test]
fn an_animation_holds_the_panel_fast_until_it_ends() {
    let at = Duration::from_millis;
    let mut pace = Pace::default();
    pace.animating(true);
    assert_eq!(pace.step(at(0)), Some(Change::Hurry));
    assert_eq!(pace.settles_at(), None);
    assert_eq!(pace.step(at(10_000)), None);
    pace.animating(false);
    assert_eq!(pace.settles_at(), Some(at(0) + QUIET));
    assert_eq!(pace.step(at(10_000)), Some(Change::Settle));
}
