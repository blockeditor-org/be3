use super::*;

#[test]
fn closing_the_canvas_right_after_a_drop_keeps_the_card() {
    let editor = a_dropped_card_takes_its_childs_size_in_the_step_that_added_it::dropped();
    let store = editor.store();
    let added = entities(&editor);
    assert_eq!(added.len(), 1, "the drop added one card");
    // Before the child's editor has told its size, so its card's undo group is still open.
    drop(editor);
    store.detach_editors(None);
    assert_eq!(
        store.content::<CanvasContent>(None).root().entities(),
        added,
        "the card stays once the canvas closes and its editor detaches"
    );
}
