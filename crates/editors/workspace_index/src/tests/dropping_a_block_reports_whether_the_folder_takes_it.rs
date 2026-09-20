use super::*;

#[test]
fn dropping_a_block_reports_whether_the_folder_takes_it() {
    let (
        Fixture {
            mut editor, host, ..
        },
        children,
    ) = editor(1);
    let middle = editor.rect().center();

    host.set_beui_drag(Some(Drag {
        position: middle,
        block_id: Uuid::new_v4(),
        block_type: Counter::TYPE_ID,
        dropped: false,
    }));
    editor.run();
    assert_eq!(host.take_drag_accepted(), Some(true));

    host.set_beui_drag(Some(Drag {
        position: middle,
        block_id: children[0],
        block_type: Counter::TYPE_ID,
        dropped: false,
    }));
    editor.run();
    assert_eq!(host.take_drag_accepted(), Some(false));
}
