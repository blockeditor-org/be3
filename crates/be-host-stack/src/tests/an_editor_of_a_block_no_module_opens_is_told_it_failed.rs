use super::*;

/// An editor that asks for a block whose type no module here serves is told the block will not
/// open, with why, rather than left loading.
#[test]
fn an_editor_of_a_block_no_module_opens_is_told_it_failed() {
    let harness = Harness::start();
    harness.connect();
    let (block, unknown_type, editor) = (Uuid::new_v4(), Uuid::new_v4(), 7_001);
    attach(block, unknown_type, editor, true);
    let failure = loop {
        let failed = take_editor_frames(editor)
            .into_iter()
            .filter(|(to, _)| *to == block)
            .filter_map(|(_, frame)| unframe(&frame))
            .filter_map(|bytes| be_sdk_type_guest::editor::ToEditor::decode(&bytes))
            .find_map(|message| match message {
                be_sdk_type_guest::editor::ToEditor::Failed(reason) => Some(reason),
                _ => None,
            });
        if let Some(reason) = failed {
            break reason;
        }
        crate::wait_for_frames("told the editor the block will not open", editor);
    };
    assert!(failure.contains("no app"), "it says why: {failure}");
}
