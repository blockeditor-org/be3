use super::*;

#[test]
fn damage_close_together_is_drawn_as_one_region() {
    let mut damage = Damage::default();
    damage.add(WindowId::MAIN, rect(0.0, 0.0, 40.0, 40.0));
    damage.add(WindowId::MAIN, rect(50.0, 50.0, 90.0, 90.0));
    damage.add(WindowId::MAIN, rect(50.0, 0.0, 90.0, 40.0));
    damage.add(WindowId::MAIN, rect(0.0, 50.0, 40.0, 90.0));
    let region = damage.take().of(WindowId::MAIN, rect(0.0, 0.0, 100.0, 100.0));

    assert_eq!(
        region.rects(),
        [rect(0.0, 0.0, 90.0, 90.0)],
        "four regions that between them cover most of their bounds are drawn as one"
    );
}
