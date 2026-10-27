use super::*;
use block_editor_beui::beui::TouchPhase;

#[test]
fn a_phone_opens_a_context_submenu_as_a_page_of_its_sheet() {
    let rectangle = card();
    let mut editor = vector_phone(std::slice::from_ref(&rectangle));
    let center = editor
        .rect_of(&format!("infinite-canvas.entity.{}", rectangle.id))
        .center();
    editor.fingers(TouchPhase::Start, &[(1, center)]);
    editor.advance(std::time::Duration::from_secs(1));
    editor.run();
    editor.fingers(TouchPhase::End, &[(1, center)]);
    assert!(editor.shown("infinite-canvas.menu.canvas.grid-menu"));

    editor.tap("infinite-canvas.menu.canvas.grid-menu");
    assert!(
        !editor.shown("infinite-canvas.menu.canvas.delete"),
        "the submenu takes the sheet's place rather than hanging beside it"
    );
    assert!(editor.shown("infinite-canvas.menu.canvas.grid-medium"));
    editor.advance(std::time::Duration::from_secs(1));
    editor.run();
    editor.snapshot("a_phone_opens_a_context_submenu_as_a_page_of_its_sheet");

    editor.tap("infinite-canvas.menu.canvas.grid-large");
    assert_eq!(
        editor.content::<CanvasContent>(None).root().grid.shown(),
        Some(infinite_canvas_types::CanvasGridSpacing::Large),
        "a row on the page runs its action"
    );
}
