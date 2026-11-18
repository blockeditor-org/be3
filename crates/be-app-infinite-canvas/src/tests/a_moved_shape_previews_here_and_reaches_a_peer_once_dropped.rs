use super::*;

#[test]
fn a_moved_shape_previews_here_and_reaches_a_peer_once_dropped() {
    let mut shape = card();
    shape.text = "Plan".into();
    let mut mover = editor(std::slice::from_ref(&shape));
    let mut watcher = editor(std::slice::from_ref(&shape));
    let item = format!("infinite-canvas.entity.{}", shape.id);
    let label = format!("infinite-canvas.text.{}", shape.id);
    let (resting, written) = (mover.rect_of(&item), mover.rect_of(&label));

    let from = resting.center();
    let by = Vec2::new(120.0, 60.0);
    mover.pen(PenPhase::Down(PointerButton::Primary), from, 0.5);
    mover.pen(PenPhase::Move, from + by / 2.0, 0.5);
    mover.pen(PenPhase::Move, from + by, 0.5);
    assert_eq!(
        mover.rect_of(&item).center(),
        resting.center() + by,
        "the move previews here, from the block itself"
    );
    assert_eq!(
        mover.rect_of(&label).center(),
        written.center() + by,
        "its label goes with it"
    );
    assert_eq!(
        entities(&mover)[0].transform.center,
        CanvasPoint::new(by.x, by.y),
        "every view of the block on this device reads the held move"
    );
    mover.snapshot("a_moved_shape_previews_here_and_reaches_a_peer_once_dropped");
    sync(&mut mover, &mut watcher);
    assert_eq!(
        watcher.rect_of(&item),
        resting,
        "nothing of a move not yet dropped is written"
    );

    mover.pen(PenPhase::Up(PointerButton::Primary), from + by, 0.0);
    sync(&mut mover, &mut watcher);
    assert_eq!(entities(&watcher), entities(&mover), "the drop is written");
    assert_eq!(watcher.rect_of(&item).center(), resting.center() + by);

    undo(&mut mover);
    assert_eq!(
        entities(&mover)[0].transform.center,
        CanvasPoint::default(),
        "one undo takes the whole move back"
    );
}
