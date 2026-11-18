use super::*;
use be_sdk_beui::beui::{Key, Modifiers};

#[test]
fn a_selection_carried_from_the_keyboard_steps_and_drops_as_one_edit() {
    let shape = card();
    let mut editor = editor(std::slice::from_ref(&shape));
    let item = format!("infinite-canvas.entity.{}", shape.id);
    editor.click(&item);
    let resting = editor.rect_of(&item);
    // Where the shape is, in world units: a step is the arrows' nudge, a world unit.
    let at = |editor: &BeuiTest<CanvasRoot>| entities(editor)[0].transform.center;
    editor.store().take_written(None);
    let shift = Modifiers {
        shift: true,
        ..Modifiers::NONE
    };
    let ctrl_shift = Modifiers {
        ctrl: true,
        shift: true,
        ..Modifiers::NONE
    };

    editor.key(Key::Space, Modifiers::NONE);
    editor.key(Key::ArrowRight, Modifiers::NONE);
    editor.key(Key::ArrowRight, shift);
    assert_eq!(
        at(&editor),
        CanvasPoint::new(11.0, 0.0),
        "Space lifts the selection and each arrow steps it on as far as it nudges, ten with Shift"
    );
    editor.key(Key::Delete, Modifiers::NONE);
    assert_eq!(
        entities(&editor).len(),
        1,
        "what is carried is not deleted from under the carry"
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
    assert_eq!(at(&editor), CanvasPoint::new(0.0, 1.0), "Enter drops it");
    assert!(
        !editor.store().take_written(None).is_empty(),
        "the drop is written"
    );

    editor.key(Key::ArrowLeft, ctrl_shift);
    assert_eq!(
        at(&editor),
        CanvasPoint::new(-1.0, 1.0),
        "Ctrl+Shift and an arrow carries it one step at once"
    );
    undo(&mut editor);
    assert_eq!(
        at(&editor),
        CanvasPoint::new(0.0, 1.0),
        "each keyboard move is one undo step"
    );
    undo(&mut editor);
    assert_eq!(editor.rect_of(&item), resting);
}
