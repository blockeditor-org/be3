use super::*;
use block_editor_beui::beui::styled::{TextAreaApi, TextAreaSurfaceApi};
use block_editor_beui::beui::{Key, Modifiers, Pos2, Rect, pos2};
use infinite_canvas_types::CanvasTextStyle;

fn zoom_to(editor: &mut BeuiTest<CanvasApp>, label: &str) {
    for _ in 0..8 {
        if editor.label("infinite-canvas.pill.zoom") == label {
            return;
        }
        editor.click("infinite-canvas.pill.zoom-in");
        editor.run();
        editor.settle();
    }
    assert_eq!(editor.label("infinite-canvas.pill.zoom"), label);
}

fn index_at(editor: &BeuiTest<CanvasApp>, field: &str, at: Pos2) -> usize {
    let document = editor.document();
    let node = document.find_test_id(field).expect("the text is drawn");
    let surface = document.api::<TextAreaSurfaceApi>(node).surface();
    document
        .api::<TextAreaApi>(surface)
        .index_at(&*document, at)
}

fn layout(editor: &BeuiTest<CanvasApp>, field: &str) -> Vec<usize> {
    let rect = editor.rect_of(field);
    let (columns, rows) = (7, 12);
    (0..rows)
        .flat_map(|row| (0..columns).map(move |column| (row, column)))
        .map(|(row, column)| {
            let x = rect.left() + rect.width() * (column as f32 + 0.5) / columns as f32;
            let y = rect.top() + rect.height() * (row as f32 + 0.5) / rows as f32;
            index_at(editor, field, pos2(x, y))
        })
        .collect()
}

#[test]
fn zoomed_text_wraps_the_same_while_edited_and_takes_typing_where_pressed() {
    let text = "The quick brown fox jumps over the lazy dog and runs off again";
    let mut note = entity(Uuid::from_u128(1));
    note.shape = CanvasShape::Text;
    note.transform =
        CanvasTransform::new(CanvasPoint::default(), CanvasPoint::new(150.0, 140.0), 0.0);
    note.text_style = CanvasTextStyle {
        wrap: true,
        ..CanvasTextStyle::default()
    };
    note.text = text.into();
    let mut editor = editor(std::slice::from_ref(&note));
    let field = format!("infinite-canvas.text.{}", note.id);
    zoom_to(&mut editor, "125%");

    let resting = editor.rect_of(&field);
    let shown = layout(&editor, &field);
    let starts = (0..12)
        .map(|row| shown[row * 7])
        .collect::<HashSet<_>>()
        .len();
    assert!(starts >= 3, "the text wraps onto several lines: {shown:?}");
    editor.double_click(pos2(resting.left() + 8.0, resting.top() + 8.0));
    editor.run();
    assert_eq!(
        editor.rect_of(&field),
        resting,
        "the text stays put to edit"
    );
    assert_eq!(
        layout(&editor, &field),
        shown,
        "the text wraps the same while it is edited"
    );
    editor.key(Key::Escape, Modifiers::NONE);
    editor.run();

    let band = editor.content_rect().expect("the canvas reports its stage");
    editor.set_view(
        Rect::from_center_size(band.center(), band.size() * 2.0),
        2.0,
    );
    editor.settle();
    assert_eq!(editor.label("infinite-canvas.pill.zoom"), "200%");
    let resting = editor.rect_of(&field);
    let pressed = pos2(resting.center().x, resting.center().y);
    let at = index_at(&editor, &field, pressed);
    assert!(0 < at && at < text.len(), "the press lands inside the text");
    editor.double_click(pressed);
    editor.run();
    editor.type_text("!");
    editor.run();
    let mut written = text.to_owned();
    written.insert(at, '!');
    assert_eq!(
        entities(&editor)[0].text.to_str_lossy(),
        written,
        "typing at the zoom lands where the text was pressed"
    );
    editor.snapshot("zoomed_text_being_edited");
}
