use super::*;

/// A drag's frames absorbed into one edit (`LiveEdit::absorb_operation`, as a held layer told
/// whole or a burst of submissions is) keep only the last write of each field: the edit is
/// no larger after many frames than after one, and lands where the last frame put it.
#[test]
fn a_drags_frames_absorbed_are_one_write_of_the_last() {
    let entity = rectangle();
    let start = canvas_run(
        &CanvasContent::default(),
        InfiniteCanvasOperation::Add {
            entity: Box::new(entity.clone()),
            pixels: None,
        },
    );
    let frame = |at: f32| {
        let mut moved = entity.clone();
        moved.transform.center = CanvasPoint::new(at, at / 2.0);
        moved.transform.size = CanvasPoint::new(100.0 + at, 80.0 + at);
        crate::Canvas::edit_in(
            &start,
            &InfiniteCanvasOperation::Update {
                entities: vec![moved],
            },
        )
    };
    let mut absorbed = frame(1.0);
    let one = CanvasContent::encode_operation(&absorbed).len();
    for at in 2..=60 {
        assert!(CanvasContent::absorb_operation(&mut absorbed, frame(at as f32)).is_none());
    }
    assert!(CanvasContent::encode_operation(&absorbed).len() <= one + 8);
    assert_eq!(
        edited(&start, [absorbed]).root().entities(),
        edited(&start, [frame(60.0)]).root().entities()
    );
}
