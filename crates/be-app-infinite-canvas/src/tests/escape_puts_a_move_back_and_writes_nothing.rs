use super::*;
use be_sdk_beui::beui::{Modifiers, PenPhase, PointerButton};

#[test]
fn escape_puts_a_move_back_and_writes_nothing() {
    let shape = card();
    let mut mover = editor(std::slice::from_ref(&shape));
    let item = format!("infinite-canvas.entity.{}", shape.id);
    let resting = mover.rect_of(&item);
    mover.store().take_written(None);

    let from = resting.center();
    mover.pen(PenPhase::Down(PointerButton::Primary), from, 0.5);
    mover.pen(PenPhase::Move, from + Vec2::new(90.0, 0.0), 0.5);
    assert_ne!(mover.rect_of(&item), resting, "the move previews");

    mover.key(be_sdk_beui::beui::Key::Escape, Modifiers::NONE);
    assert_eq!(mover.rect_of(&item), resting, "Escape puts it back");
    mover.pen(
        PenPhase::Up(PointerButton::Primary),
        from + Vec2::new(90.0, 0.0),
        0.0,
    );
    assert_eq!(entities(&mover), vec![shape]);
    assert!(
        mover.store().take_written(None).is_empty(),
        "a move called off leaves no trace"
    );
}
