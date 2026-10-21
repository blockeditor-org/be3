use super::*;

#[test]
fn a_long_stroke_is_sent_in_parts_and_put_back_together() {
    let id = Uuid::from_u128(9);
    let style = CanvasStyle::default();
    let points: Vec<_> = (0..STROKE_PART * 2 + 5)
        .map(|index| CanvasPoint::new(index as f32, (index * 2) as f32))
        .collect();
    let pressures: Vec<_> = (0..points.len())
        .map(|index| index as f32 / 100.0)
        .collect();

    let parts = stroke_parts(id, style, &points, &pressures, 0);
    assert_eq!(
        parts.len(),
        3,
        "each part holds at most {STROKE_PART} points"
    );
    let later = stroke_parts(id, style, &points, &pressures, 2);
    assert_eq!(
        later,
        parts[2..],
        "a part is rebuilt alone when only it grew"
    );

    let mut peers: Vec<_> = parts.into_iter().map(|part| (7, part)).collect();
    peers.reverse();
    let drafts = assemble(&peers);
    assert_eq!(
        drafts,
        [(
            7,
            Draft::Stroke {
                id,
                points: points.clone(),
                pressures: pressures.clone(),
                style,
            }
        )]
    );

    peers.retain(|(_, draft)| !matches!(draft, CanvasDraft::Stroke { part: 1, .. }));
    let Draft::Stroke { points: held, .. } = &assemble(&peers)[0].1 else {
        panic!("the stroke is still a stroke");
    };
    assert_eq!(
        held[..],
        points[..STROKE_PART],
        "a missing part ends the stroke there"
    );
}
