use super::*;
use be_sdk_beui::beui::Modifiers;

#[test]
fn a_duplicating_move_writes_its_copies_as_it_goes() {
    let shape = card();
    let mut mover = editor(std::slice::from_ref(&shape));
    let mut watcher = editor(std::slice::from_ref(&shape));
    let from = mover
        .rect_of(&format!("infinite-canvas.entity.{}", shape.id))
        .center();

    mover.hold_modifiers(Modifiers {
        alt: true,
        ..Modifiers::NONE
    });
    mover.pen(PenPhase::Down(PointerButton::Primary), from, 0.5);
    mover.pen(PenPhase::Move, from + Vec2::new(80.0, 0.0), 0.5);
    sync(&mut mover, &mut watcher);
    assert_eq!(
        entities(&watcher).len(),
        2,
        "a copy is made as the move starts, so it is written as it goes, as a stroke is"
    );
    mover.pen(PenPhase::Up(PointerButton::Primary), from + Vec2::new(80.0, 0.0), 0.0);
    sync(&mut mover, &mut watcher);
    assert_eq!(entities(&watcher), entities(&mover));

    undo(&mut mover);
    assert_eq!(
        entities(&mover),
        vec![shape],
        "one undo takes the whole duplicating move back"
    );
}
