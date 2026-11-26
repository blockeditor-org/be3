use super::*;
use be_sdk_beui::beui::TouchPhase;

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
    // The menu's sheet rises from below to the middle of the screen; a slow swipe up scrolls
    // what it holds, and its lower rows lay out as they come into reach.
    editor.settle();
    let from = editor.rect_of("sheet.handle").center() + be_sdk_beui::beui::vec2(0.0, 40.0);
    editor.fingers(TouchPhase::Start, &[(1, from)]);
    editor.run();
    for step in 1..=15 {
        let at = from - be_sdk_beui::beui::vec2(0.0, step as f32 * 15.0);
        editor.fingers(TouchPhase::Move, &[(1, at)]);
        editor.run();
    }
    editor.advance(std::time::Duration::from_millis(200));
    editor.run();
    editor.fingers(
        TouchPhase::End,
        &[(1, from - be_sdk_beui::beui::vec2(0.0, 225.0))],
    );
    editor.settle();
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
        Some(be_type_infinite_canvas::CanvasGridSpacing::Large),
        "a row on the page runs its action"
    );
}
