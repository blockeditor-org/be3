use super::*;

#[test]
fn damage_covers_a_shape_inserted_between_unchanged_neighbours() {
    let first = filled(rect(0.0, 0.0, 10.0, 10.0));
    let last = filled(rect(80.0, 80.0, 90.0, 90.0));
    let before = [first.clone(), last.clone()];
    let after = [first, filled(rect(40.0, 40.0, 50.0, 50.0)), last];

    assert_eq!(between(&before, &after), rect(40.0, 40.0, 50.0, 50.0));
}
