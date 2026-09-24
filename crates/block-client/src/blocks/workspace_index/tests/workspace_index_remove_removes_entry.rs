use block::Block;
use uuid::Uuid;

use super::{WorkspaceIndex, WorkspaceIndexOperation};

#[test]
fn workspace_index_remove_removes_entry() {
    let entry = Uuid::new_v4();
    let mut index = WorkspaceIndex::default();

    WorkspaceIndex::apply_operation(&mut index, &WorkspaceIndexOperation::Add(entry));
    WorkspaceIndex::apply_operation(&mut index, &WorkspaceIndexOperation::Remove(entry));

    assert!(index.entries().is_empty());
}
