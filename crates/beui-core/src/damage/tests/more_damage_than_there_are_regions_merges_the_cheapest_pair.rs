use super::*;

#[test]
fn more_damage_than_there_are_regions_merges_the_cheapest_pair() {
    let mut damage = Damage::default();
    for column in 0..8 {
        let left = column as f32 * 20.0;
        damage.add(rect(left, 0.0, left + 5.0, 5.0));
    }
    damage.add(rect(6.0, 0.0, 10.0, 5.0));
    let region = damage.take(rect(0.0, 0.0, 200.0, 100.0));

    assert_eq!(region.rects().len(), 8);
    assert!(
        region.rects().contains(&rect(0.0, 0.0, 10.0, 5.0)),
        "the ninth region joins whichever region it grows least: {:?}",
        region.rects()
    );
}
