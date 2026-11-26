use super::*;

/// A block's projection takes all of its updates, so it is held as one content type: asking
/// for it as another is a bug.
#[test]
#[should_panic(expected = "another content type")]
fn a_block_is_projected_as_one_content_type() {
    let host = EditorHost::default();
    let block = uuid::Uuid::new_v4();
    let first = crate::EditorCore::new(host.clone(), uuid::Uuid::new_v4());
    let second = crate::EditorCore::new(host, uuid::Uuid::new_v4());
    let _held = first.content_of::<ChecklistContent>(block);
    let _ = second.content_of::<be_block_content::EditorViewContent>(block);
}
