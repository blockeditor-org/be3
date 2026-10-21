use super::*;

#[test]
fn a_stroke_that_has_landed_is_not_drawn_again() {
    let id = Uuid::from_u128(9);
    let stroke = Draft::Stroke {
        id,
        points: vec![CanvasPoint::new(1.0, 2.0)],
        pressures: Vec::new(),
        style: CanvasStyle::default(),
    };
    let mut landed = HashSet::new();

    let drawn = unlanded(vec![(7, stroke.clone())], |_| false, &mut landed);
    assert_eq!(drawn.len(), 1, "a stroke still being drawn is drawn");

    let drawn = unlanded(vec![(7, stroke.clone())], |of| of == id, &mut landed);
    assert!(
        drawn.is_empty(),
        "once its entity arrives only the entity is drawn"
    );

    let drawn = unlanded(vec![(7, stroke.clone())], |_| false, &mut landed);
    assert!(
        drawn.is_empty(),
        "undoing the entity before the peer takes its stroke down does not bring the stroke back"
    );

    let drawn = unlanded(Vec::new(), |_| false, &mut landed);
    assert!(drawn.is_empty());
    assert!(landed.is_empty(), "a stroke taken down is forgotten");
}
