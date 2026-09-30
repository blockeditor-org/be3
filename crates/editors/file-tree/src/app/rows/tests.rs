use std::collections::HashMap;

use block_editor_beui::EditorHost;
use block_editor_beui::block_ui::{BlockCatalog, BlockTypeEntry, ChildEdits};

use super::*;

mod access_marker_marks_limited_access;
mod deleting_needs_a_container_that_can_delete_children;
mod unlinking_needs_a_container_that_can_replace_a_child;

const CONTAINER: Uuid = Uuid::from_u128(1);
const LISTED: Uuid = Uuid::from_u128(2);

fn catalog(edits: ChildEdits) -> BlockCatalog {
    BlockCatalog::new([(
        LISTED,
        BlockTypeEntry {
            display_name: "Listed".to_owned(),
            icon: None,
            child_edits: edits,
        },
    )])
}

fn block_types() -> HashMap<Uuid, Uuid> {
    [(CONTAINER, LISTED)].into_iter().collect()
}

fn client() -> Blocks {
    let host = EditorHost::default();
    for id in [CONTAINER, LISTED] {
        host.set_blocks(
            BlockQuery::Block(id),
            vec![BlockInfo::new(id, LISTED, BlockParent::Root)],
        );
    }
    host.blocks()
}
