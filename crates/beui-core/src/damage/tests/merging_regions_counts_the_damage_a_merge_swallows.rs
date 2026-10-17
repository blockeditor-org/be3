use super::*;

#[test]
fn merging_regions_counts_the_damage_a_merge_swallows() {
    let mut damage = Damage::default();
    damage.add(rect(0.0, 0.0, 1.0, 100.0));
    damage.add(rect(0.0, 99.0, 100.0, 100.0));
    damage.add(rect(1.0, 80.0, 100.0, 99.0));
    for column in 0..7 {
        let left = 200.0 + column as f32 * 50.0;
        damage.add(rect(left, 0.0, left + 10.0, 10.0));
    }
    let region = damage.take(rect(0.0, 0.0, 700.0, 200.0));

    assert!(
        !region
            .rects()
            .iter()
            .any(|damaged| damaged.intersects(rect(10.0, 10.0, 90.0, 70.0))),
        "joining the bottom rows would swallow the strip along the left and fill the square: {:?}",
        region.rects()
    );
}
