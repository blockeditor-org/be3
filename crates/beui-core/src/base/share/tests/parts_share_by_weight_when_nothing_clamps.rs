use super::*;

#[test]
fn parts_share_by_weight_when_nothing_clamps() {
    let parts = [
        Part::new(1.0, 0.0, f32::INFINITY),
        Part::new(3.0, 0.0, f32::INFINITY),
    ];

    assert_eq!(share(100.0, &parts), vec![25.0, 75.0]);
}
