use super::*;

#[test]
fn more_damage_than_there_are_regions_merges_the_cheapest_pair() {
    let mut damage = Damage::default();
    for column in 0..4 {
        let left = column as f32 * 30.0;
        damage.add(rect(left, 0.0, left + 5.0, 5.0));
    }
    damage.add(rect(6.0, 0.0, 10.0, 5.0));
    let region = damage.take(rect(0.0, 0.0, 100.0, 100.0));

    let mut rects = region.rects().to_vec();
    rects.sort_by(|a, b| a.left().total_cmp(&b.left()));
    assert_eq!(
        rects,
        [
            rect(0.0, 0.0, 10.0, 5.0),
            rect(30.0, 0.0, 35.0, 5.0),
            rect(60.0, 0.0, 65.0, 5.0),
            rect(90.0, 0.0, 95.0, 5.0),
        ],
        "the fifth region joins whichever of the four it grows least"
    );
}
