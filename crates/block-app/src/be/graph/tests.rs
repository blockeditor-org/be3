use super::*;

mod an_older_announcement_never_undoes_a_newer_one;

fn node(id: Uuid) -> Node {
    Node {
        id,
        content_type: Uuid::nil(),
        author: None,
        parent: Some(BlockParent::Root),
        access: Access::Edit,
        references: Vec::new(),
        metadata: BlockMetadata::default(),
        head: None,
        locked: false,
    }
}
