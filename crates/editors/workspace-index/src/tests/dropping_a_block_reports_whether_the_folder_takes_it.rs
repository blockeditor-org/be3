use super::*;

#[test]
fn dropping_a_block_reports_whether_the_folder_takes_it() {
    let (Fixture { mut editor, .. }, children) = editor(1);
    let middle = editor.rect().center();

    editor.drag_block(middle, Uuid::new_v4(), CounterContent::CONTENT_TYPE, false);
    editor.run();
    assert_eq!(editor.take_drag_accepted(), Some(true));

    editor.drag_block(middle, children[0], CounterContent::CONTENT_TYPE, false);
    editor.run();
    assert_eq!(editor.take_drag_accepted(), Some(false));
}
