use super::*;

#[test]
fn an_unchanged_paint_list_has_no_damage() {
    let shapes = [
        filled(rect(0.0, 0.0, 10.0, 10.0)),
        filled(rect(20.0, 20.0, 30.0, 30.0)),
    ];

    assert!(!between(&shapes, &shapes).is_positive());
}
