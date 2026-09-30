use super::*;

#[test]
fn damage_that_overlaps_what_is_held_merges_into_it() {
    let mut damage = Damage::default();
    damage.add(rect(10.0, 10.0, 20.0, 20.0));
    damage.add(rect(15.0, 15.0, 40.0, 25.0));
    let region = damage.take(rect(0.0, 0.0, 100.0, 100.0));

    assert_eq!(region.rects(), [rect(10.0, 10.0, 40.0, 25.0)]);
}
