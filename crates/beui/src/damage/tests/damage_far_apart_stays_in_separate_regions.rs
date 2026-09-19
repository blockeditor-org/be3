use super::*;

#[test]
fn damage_far_apart_stays_in_separate_regions() {
    let mut damage = Damage::default();
    damage.add(rect(10.0, 10.0, 20.0, 20.0));
    damage.add(rect(40.0, 5.0, 50.0, 30.0));
    damage.add(Rect::NOTHING);
    let region = damage.take(rect(0.0, 0.0, 100.0, 100.0));

    assert_eq!(
        region.rects(),
        [rect(10.0, 10.0, 20.0, 20.0), rect(40.0, 5.0, 50.0, 30.0)]
    );
    assert!(
        !region.intersects(rect(25.0, 10.0, 35.0, 20.0)),
        "what lies between two damaged regions is not itself damaged"
    );
}
