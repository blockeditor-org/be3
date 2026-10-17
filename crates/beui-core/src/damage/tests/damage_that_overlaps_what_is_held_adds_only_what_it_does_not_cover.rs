use super::*;

#[test]
fn damage_that_overlaps_what_is_held_adds_only_what_it_does_not_cover() {
    let mut damage = Damage::default();
    damage.add(rect(0.0, 0.0, 100.0, 10.0));
    damage.add(rect(90.0, 0.0, 100.0, 100.0));
    let region = damage.take(rect(0.0, 0.0, 200.0, 200.0));

    assert_eq!(
        region.rects(),
        [rect(0.0, 0.0, 100.0, 10.0), rect(90.0, 10.0, 100.0, 100.0)],
        "an L of damage leaves the square it bounds undamaged"
    );
}
