use super::*;

/// Two editors of one instance reading the same block hold one projection of it, so both read
/// every update the host sends for it, however they take turns pumping.
#[test]
fn two_editors_of_one_instance_share_a_blocks_projection() {
    let host = EditorHost::default();
    let block = uuid::Uuid::new_v4();
    let first = crate::EditorCore::new(host.clone(), uuid::Uuid::new_v4());
    let second = crate::EditorCore::new(host.clone(), uuid::Uuid::new_v4());
    let one = first.content_of::<ChecklistContent>(block);
    let other = second.content_of::<ChecklistContent>(block);
    assert!(one == other);

    let (_, add) = Checklist::add("shared");
    let mut content = ChecklistContent::default();
    content.apply(&add);
    host.set_content_of(block, HostContent::of(&content));
    other.pump();
    let listed = |projection: &LiveContent<ChecklistContent>| {
        projection
            .read(|content| content.root().items.len())
            .unwrap_or_default()
    };
    assert_eq!(listed(&one), 1);
    assert_eq!(listed(&other), 1);
}
