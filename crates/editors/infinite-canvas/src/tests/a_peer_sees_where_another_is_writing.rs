use super::*;
use crate::presence::CanvasCursor;
use block_editor_beui::beui::styled::{RemoteTextCursor, TextAreaApi};
use block_editor_beui::beui::{Key, Modifiers, pos2};

fn shown_carets(editor: &BeuiTest<CanvasApp>, id: Uuid) -> Vec<RemoteTextCursor> {
    let area = editor
        .document()
        .find_test_id(&format!("infinite-canvas.text.{id}"))
        .expect("the text is drawn");
    editor
        .document()
        .api::<TextAreaApi>(area)
        .remote_cursors()
}

fn shown_cursor(editor: &mut BeuiTest<CanvasApp>) -> Option<CanvasCursor> {
    editor
        .take_shown_presence()
        .into_iter()
        .rev()
        .find(|shown| shown.kind == CanvasCursor::ID)
        .and_then(|shown| serde_json::from_slice(&shown.value?).ok())
}

#[test]
fn a_peer_sees_where_another_is_writing() {
    let mut note = entity(Uuid::from_u128(1));
    note.shape = CanvasShape::Text;
    note.transform =
        CanvasTransform::new(CanvasPoint::default(), CanvasPoint::new(220.0, 40.0), 0.0);
    note.text = "Hello world".into();
    let mut shape = card();
    shape.id = Uuid::from_u128(2);
    shape.transform.center = CanvasPoint::new(0.0, 140.0);
    shape.text = "Box".into();
    let mut writer = editor(&[note.clone(), shape.clone()]);
    let mut watcher = editor(&[note.clone(), shape.clone()]);
    writer.presence_visible(true);
    writer.run();
    let mut held = BTreeMap::new();

    let resting = writer.rect_of(&format!("infinite-canvas.text.{}", note.id));
    writer.double_click(pos2(resting.right() - 2.0, resting.center().y));
    writer.run();
    for _ in 0.."world".len() {
        writer.key(Key::ArrowLeft, Modifiers::SHIFT);
    }
    writer.run();
    forward_presence(&mut writer, &mut watcher, &mut held);
    let [theirs] = &shown_carets(&watcher, note.id)[..] else {
        panic!("the text nobody here edits draws the peer's caret");
    };
    assert_eq!(
        (theirs.selection.clone(), theirs.caret),
        (6..11, 6),
        "the peer's selection and caret land where they made them"
    );
    assert!(shown_carets(&watcher, shape.id).is_empty());
    watcher.snapshot("a_peer_sees_where_another_is_writing");

    writer.key(Key::Escape, Modifiers::NONE);
    writer.run();
    let cursor = shown_cursor(&mut writer).expect("the writer shows its cursor");
    assert_eq!(cursor.caret, None, "a text left behind shares no caret");
    held.clear();
    held.insert(CanvasCursor::ID, serde_json::to_vec(&cursor).unwrap());
    forward_presence(&mut writer, &mut watcher, &mut held);
    assert!(shown_carets(&watcher, note.id).is_empty());

    writer.click(format!("infinite-canvas.entity.{}", shape.id));
    writer.run();
    writer.key(Key::Enter, Modifiers::NONE);
    writer.run();
    forward_presence(&mut writer, &mut watcher, &mut held);
    let label = shape.text.to_str_lossy().len();
    let [theirs] = &shown_carets(&watcher, shape.id)[..] else {
        panic!("a label being edited draws the peer's caret");
    };
    assert_eq!(
        (theirs.selection.clone(), theirs.caret),
        (0..label, label),
        "entering a label selects all of it, and the peer sees that"
    );
}
