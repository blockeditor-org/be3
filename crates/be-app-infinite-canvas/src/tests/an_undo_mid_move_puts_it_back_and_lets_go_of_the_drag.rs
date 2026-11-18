use super::*;

#[test]
fn an_undo_mid_move_puts_it_back_and_lets_go_of_the_drag() {
    let shape = card();
    let mut editor = editor(std::slice::from_ref(&shape));
    let item = format!("infinite-canvas.entity.{}", shape.id);
    let resting = editor.rect_of(&item);
    editor.store().take_written(None);

    let from = resting.center();
    editor.pen(PenPhase::Down(PointerButton::Primary), from, 0.5);
    editor.pen(PenPhase::Move, from + Vec2::new(60.0, 0.0), 0.5);
    assert_ne!(editor.rect_of(&item), resting);

    undo(&mut editor);
    assert_eq!(
        editor.rect_of(&item),
        resting,
        "the undo calls the held move off"
    );
    editor.pen(PenPhase::Move, from + Vec2::new(90.0, 0.0), 0.5);
    assert_eq!(
        editor.rect_of(&item),
        resting,
        "and the canvas lets go of the drag, so the pointer carries nothing"
    );
    editor.pen(
        PenPhase::Up(PointerButton::Primary),
        from + Vec2::new(90.0, 0.0),
        0.0,
    );
    assert_eq!(entities(&editor), vec![shape]);
    assert!(editor.store().take_written(None).is_empty());
}
