use super::*;

#[test]
fn more_damage_than_there_are_regions_merges_the_cheapest_pair() {
    let mut damage = Damage::default();
    for column in 0..4 {
        let left = column as f32 * 20.0;
        damage.add(rect(left, 0.0, left + 5.0, 5.0));
    }
    damage.add(rect(6.0, 0.0, 10.0, 5.0));
    let region = damage.take(rect(0.0, 0.0, 100.0, 100.0));

    assert_eq!(
        region.rects(),
        [
            rect(0.0, 0.0, 10.0, 5.0),
            rect(20.0, 0.0, 25.0, 5.0),
            rect(40.0, 0.0, 45.0, 5.0),
            rect(60.0, 0.0, 65.0, 5.0),
        ],
        "the fifth region has to join whichever of the four it grows least"
    );
}
