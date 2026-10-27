use super::*;
use block_editor_beui::beui::Modifiers;

#[test]
fn a_peer_sees_a_move_called_off_put_back() {
    let shape = card();
    let mut mover = editor(std::slice::from_ref(&shape));
    let mut watcher = editor(std::slice::from_ref(&shape));
    let item = format!("infinite-canvas.entity.{}", shape.id);
    let resting = watcher.rect_of(&item);

    let from = mover.rect_of(&item).center();
    mover.pen(PenPhase::Down(PointerButton::Primary), from, 0.5);
    mover.next_frame();
    mover.pen(PenPhase::Move, from + Vec2::new(90.0, 0.0), 0.5);
    mover.run();
    sync(&mut mover, &mut watcher);
    assert_ne!(watcher.rect_of(&item), resting);

    mover.key(block_editor_beui::beui::Key::Escape, Modifiers::NONE);
    sync(&mut mover, &mut watcher);
    assert_eq!(watcher.rect_of(&item), resting);
    assert_eq!(mover.rect_of(&item).center(), from);
    mover.pen(
        PenPhase::Up(PointerButton::Primary),
        from + Vec2::new(90.0, 0.0),
        0.0,
    );
    mover.run();
    assert_eq!(entities(&mover), vec![shape.clone()]);
    assert_eq!(entities(&watcher), vec![shape]);
}
