use super::*;

#[test]
fn a_peer_sees_a_labelled_shape_while_it_is_moved() {
    let mut shape = card();
    shape.text = "Plan".into();
    let mut mover = editor(std::slice::from_ref(&shape));
    let mut watcher = editor(std::slice::from_ref(&shape));
    let item = format!("infinite-canvas.entity.{}", shape.id);
    let label = format!("infinite-canvas.text.{}", shape.id);
    let (resting, written) = (watcher.rect_of(&item), watcher.rect_of(&label));

    let from = mover.rect_of(&item).center();
    let by = Vec2::new(120.0, 60.0);
    mover.pen(PenPhase::Down(PointerButton::Primary), from, 0.5);
    mover.next_frame();
    mover.pen(PenPhase::Move, from + by / 2.0, 0.5);
    mover.run();
    sync(&mut mover, &mut watcher);
    assert_eq!(
        watcher.rect_of(&item).center(),
        resting.center() + by / 2.0,
        "the peer draws the shape where it is so far"
    );

    mover.next_frame();
    mover.pen(PenPhase::Move, from + by, 0.5);
    mover.run();
    sync(&mut mover, &mut watcher);
    assert_eq!(
        watcher.rect_of(&item).center(),
        resting.center() + by,
        "the peer draws the shape where it is being moved"
    );
    assert_eq!(
        watcher.rect_of(&label).center(),
        written.center() + by,
        "its label goes with it"
    );
    watcher.snapshot("a_peer_sees_a_labelled_shape_while_it_is_moved");

    mover.pen(PenPhase::Up(PointerButton::Primary), from + by, 0.0);
    mover.run();
    sync(&mut mover, &mut watcher);
    assert_eq!(
        entities(&mover)[0].transform.center,
        CanvasPoint::new(by.x, by.y),
        "the move is written"
    );
    assert_eq!(entities(&watcher), entities(&mover));
}
