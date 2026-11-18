use super::*;
use be_sdk_beui::beui::drag::FREE_STEP;
use be_sdk_beui::beui::{Key, Modifiers};

#[test]
fn a_selection_carried_from_the_keyboard_steps_and_drops_as_one_edit() {
    let shape = card();
    let mut editor = editor(std::slice::from_ref(&shape));
    let item = format!("infinite-canvas.entity.{}", shape.id);
    editor.click(&item);
    let resting = editor.rect_of(&item);
    editor.store().take_written(None);
    let ctrl_shift = Modifiers {
        ctrl: true,
        shift: true,
        ..Modifiers::NONE
    };

    editor.key(Key::Space, Modifiers::NONE);
    editor.key(Key::ArrowRight, Modifiers::NONE);
    editor.key(Key::ArrowRight, Modifiers::NONE);
    assert_eq!(
        editor.rect_of(&item).center(),
        resting.center() + Vec2::new(2.0 * FREE_STEP, 0.0),
        "Space lifts the selection and each arrow steps it on"
    );
    assert!(
        editor.store().take_written(None).is_empty(),
        "nothing is written while it is carried"
    );
    editor.key(Key::Escape, Modifiers::NONE);
    assert_eq!(editor.rect_of(&item), resting, "Escape puts it back");
    assert!(editor.store().take_written(None).is_empty());

    editor.key(Key::Space, Modifiers::NONE);
    editor.key(Key::ArrowDown, Modifiers::NONE);
    editor.key(Key::Enter, Modifiers::NONE);
    let dropped = resting.center() + Vec2::new(0.0, FREE_STEP);
    assert_eq!(editor.rect_of(&item).center(), dropped, "Enter drops it");
    assert!(
        !editor.store().take_written(None).is_empty(),
        "the drop is written"
    );

    editor.key(Key::ArrowLeft, ctrl_shift);
    assert_eq!(
        editor.rect_of(&item).center(),
        dropped - Vec2::new(FREE_STEP, 0.0),
        "Ctrl+Shift and an arrow moves it one step at once"
    );
    undo(&mut editor);
    assert_eq!(
        editor.rect_of(&item).center(),
        dropped,
        "each keyboard move is one undo step"
    );
    undo(&mut editor);
    assert_eq!(editor.rect_of(&item), resting);
}
