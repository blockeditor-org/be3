use std::collections::HashMap;

use block_editor_beui::block_ui::{BlockCatalog, BlockTypeEntry};
use block_editor_beui::{ChildOperations, EditorHost, Press};

use super::*;

mod a_listing_says_loading_only_after_a_grace_and_shows_a_failure;
mod access_marker_marks_limited_access;
mod deleting_needs_a_container_that_can_delete_children;
mod icon_overrides_keep_lock_and_deleted_markers;
mod unlinking_needs_a_container_that_can_replace_a_child;

const CONTAINER: Uuid = Uuid::from_u128(1);
const LISTED: Uuid = Uuid::from_u128(2);

fn catalog(children: ChildOperations) -> BlockCatalog {
    BlockCatalog::new([(
        LISTED,
        BlockTypeEntry {
            display_name: "Listed".to_owned(),
            color: None,
            icon: None,
            children,
            press: Press::Open,
            plain_file: false,
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
