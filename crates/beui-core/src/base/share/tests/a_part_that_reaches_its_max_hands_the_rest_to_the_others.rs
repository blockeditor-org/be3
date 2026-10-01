use super::*;

#[test]
fn a_part_that_reaches_its_max_hands_the_rest_to_the_others() {
    let parts = [
        Part::new(1.0, 0.0, 20.0),
        Part::new(1.0, 0.0, f32::INFINITY),
        Part::new(1.0, 0.0, f32::INFINITY),
    ];

    assert_eq!(share(100.0, &parts), vec![20.0, 40.0, 40.0]);
}
