use super::*;

#[test]
fn a_peer_stops_drawing_a_move_that_is_called_off() {
    let shape = card();
    let mut mover = editor(std::slice::from_ref(&shape));
    let mut watcher = editor(std::slice::from_ref(&shape));
    mover.presence_visible(true);
    mover.run();
    let mut held = BTreeMap::new();
    let item = format!("infinite-canvas.entity.{}", shape.id);
    let resting = watcher.rect_of(&item);

    let from = mover.rect_of(&item).center();
    mover.pen(PenPhase::Down(PointerButton::Primary), from, 0.5);
    mover.next_frame();
    mover.pen(PenPhase::Move, from + Vec2::new(90.0, 0.0), 0.5);
    mover.run();
    forward_presence(&mut mover, &mut watcher, &mut held);
    assert_ne!(watcher.rect_of(&item), resting);

    mover.key_press(block_editor_beui::beui::Key::Escape);
    mover.run();
    forward_presence(&mut mover, &mut watcher, &mut held);
    assert!(
        !held.keys().any(|kind| CanvasDraft::holds(*kind)),
        "a move called off is drafted no longer"
    );
    assert_eq!(watcher.rect_of(&item), resting);
    mover.pen(
        PenPhase::Up(PointerButton::Primary),
        from + Vec2::new(90.0, 0.0),
        0.0,
    );
    mover.run();
    assert_eq!(entities(&mover)[0].transform.center, CanvasPoint::default());
}
