use super::*;

#[test]
fn damage_covers_the_old_and_new_bounds_of_a_moved_shape() {
    let background = filled(rect(0.0, 0.0, 100.0, 100.0));
    let before = [background.clone(), filled(rect(10.0, 10.0, 20.0, 20.0))];
    let after = [background, filled(rect(30.0, 30.0, 40.0, 40.0))];

    assert_eq!(between(&before, &after), rect(10.0, 10.0, 40.0, 40.0));
}
