use super::*;
use block_editor_beui::beui::pos2;
use std::time::Duration;

#[test]
fn a_drag_in_text_being_edited_selects_it_and_leaves_the_text_in_place() {
    let mut note = entity(Uuid::from_u128(1));
    note.transform =
        CanvasTransform::new(CanvasPoint::default(), CanvasPoint::new(240.0, 40.0), 0.0);
    note.shape = CanvasShape::Text;
    note.text = "Hello world".into();
    let mut editor = editor(std::slice::from_ref(&note));
    let field = format!("infinite-canvas.text.{}", note.id);
    let rect = editor.rect_of(&field);

    editor.double_click(pos2(rect.right() - 2.0, rect.center().y));
    editor.run();
    assert_eq!(
        entities(&editor)[0].transform,
        note.transform,
        "starting to edit text leaves its box as it was"
    );
    let rect = editor.rect_of(&field);
    let y = rect.center().y;

    editor.advance(Duration::from_secs(1));
    editor.drag(pos2(rect.left() + 2.0, y), pos2(rect.right() - 2.0, y));
    editor.run();
    assert_eq!(
        entities(&editor)[0].transform,
        note.transform,
        "a drag in the text being edited leaves it where it is"
    );
    editor.type_text("Hi there");
    editor.run();
    assert_eq!(
        entities(&editor)[0].text.to_str_lossy(),
        "Hi there",
        "the drag selected the text it crossed"
    );

    let rect = editor.rect_of(&field);
    editor.advance(Duration::from_secs(1));
    editor.double_click(pos2(rect.left() + 12.0, rect.center().y));
    editor.run();
    editor.type_text("you");
    editor.run();
    assert_eq!(
        entities(&editor)[0].text.to_str_lossy(),
        "you there",
        "a double-click in the text being edited selects a word"
    );

    let rect = editor.rect_of(&field);
    editor.advance(Duration::from_secs(1));
    for _ in 0..3 {
        editor.click(pos2(rect.left() + 4.0, rect.center().y));
    }
    editor.run();
    editor.type_text("All");
    editor.run();
    assert_eq!(
        entities(&editor)[0].text.to_str_lossy(),
        "All",
        "a triple-click in the text being edited selects all of it"
    );

    editor.advance(Duration::from_secs(1));
    editor.click(rect.center() + Vec2::new(0.0, 200.0));
    editor.run();
    editor.type_text("?");
    editor.run();
    assert_eq!(
        entities(&editor)[0].text.to_str_lossy(),
        "All",
        "a press outside the text stops editing it"
    );
}
