use super::*;

#[test]
fn damage_that_overlaps_what_is_held_adds_only_what_it_does_not_cover() {
    let mut damage = Damage::default();
    damage.add(rect(10.0, 10.0, 20.0, 20.0));
    damage.add(rect(15.0, 15.0, 40.0, 25.0));
    let region = damage.take(rect(0.0, 0.0, 100.0, 100.0));

    assert_eq!(
        region.rects(),
        [
            rect(10.0, 10.0, 20.0, 20.0),
            rect(20.0, 15.0, 40.0, 20.0),
            rect(15.0, 20.0, 40.0, 25.0),
        ],
        "the corner the two leave out is not damaged"
    );
}
