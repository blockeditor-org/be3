use super::*;

#[test]
fn a_painting_drawn_at_another_size_is_cropped_rather_than_stretched() {
    let whole = Rect::from_min_max(Pos2::ZERO, pos2(1.0, 1.0));
    let slot = Quad::upright(Rect::from_min_size(pos2(10.0, 20.0), vec2(100.0, 50.0)));

    let (shrunk, source) = unstretched(slot, whole, (100, 50), (100, 80))
        .expect("a taller painting still covers the slot");
    assert_eq!(
        (shrunk.rect, source),
        (
            slot.rect,
            Rect::from_min_max(Pos2::ZERO, pos2(1.0, 50.0 / 80.0))
        ),
        "a slot that shrank shows the top of the painting at its own size"
    );

    let (grown, source) = unstretched(slot, whole, (100, 50), (60, 50))
        .expect("a narrower painting covers part of the slot");
    assert_eq!(
        (grown.rect, source),
        (
            Rect::from_min_size(pos2(10.0, 20.0), vec2(60.0, 50.0)),
            whole
        ),
        "a slot that grew shows the whole painting at its own size, from its corner"
    );

    let right = Rect::from_min_max(pos2(0.5, 0.0), pos2(1.0, 1.0));
    assert!(
        unstretched(slot, right, (100, 50), (40, 50)).is_none(),
        "a piece the painting does not reach yet is left out"
    );
}
