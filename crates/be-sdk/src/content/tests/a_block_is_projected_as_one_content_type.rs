use super::*;

/// A block's projection takes all of its updates, so it is held as one content type: asked for
/// as another (block data can name a block of another type), it reads as failed, and the one
/// held goes on reading the block.
#[test]
fn a_block_is_projected_as_one_content_type() {
    let host = EditorHost::default();
    let block = uuid::Uuid::new_v4();
    let first = crate::EditorCore::new(host.clone(), uuid::Uuid::new_v4());
    let second = crate::EditorCore::new(host.clone(), uuid::Uuid::new_v4());
    let held = first.content_of::<ChecklistContent>(block);
    let other = second.content_of::<be_block_content::EditorViewContent>(block);
    let again = first.content_of::<be_block_content::EditorViewContent>(block);

    let (_, add) = Checklist::add("kept");
    let mut content = ChecklistContent::default();
    content.apply(&add);
    host.set_content_of(block, HostContent::of(&content));
    other.pump();
    again.pump();
    held.pump();
    for refused in [&other, &again] {
        assert!(matches!(
            refused.read(|_| ()),
            Err(crate::Unready::Failed(reason)) if reason.contains("another content type")
        ));
    }
    assert_eq!(
        held.read(|content| content.root().items.len()).unwrap_or_default(),
        1
    );
}
