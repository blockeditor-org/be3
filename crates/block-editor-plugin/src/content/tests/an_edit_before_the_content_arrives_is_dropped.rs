use super::*;
use crate::Unready;

#[test]
fn an_edit_before_the_content_arrives_is_dropped() {
    let host = EditorHost::default();
    host.set_editable(true);
    let projection = LiveContent::<ChecklistContent>::new(host.clone(), None);
    let (_, add) = Checklist::add("typed into nothing");

    projection.operate(add);

    assert!(host.take_content_operations().is_empty());
    assert_eq!(
        projection.read(|content| content.root().items.len()),
        Err(Unready::Loading)
    );

    let (_, kept) = Checklist::add("milk");
    let mut content = ChecklistContent::default();
    content.apply(&kept);
    host.set_block_content(HostContent::of(&content));
    projection.pump();

    assert_eq!(projection.read(|content| content.root().items.len()), Ok(1));
}
