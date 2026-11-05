use super::*;
use crate::workspace::{Workspace, WorkspaceContent};
use be_block::{ChildChange, Root};
use uuid::Uuid;

#[test]
fn a_workspace_reorders_its_children_like_a_folder() {
    let (first, second, third) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
    let workspace = edited(
        &WorkspaceContent::default(),
        [Workspace::default().add(first)],
    );
    let workspace = edited(&workspace, [workspace.root().add(second)]);
    let workspace = edited(&workspace, [workspace.root().add(third)]);
    assert_eq!(workspace.references(), vec![first, second, third]);

    let moved = edited(
        &workspace,
        workspace.root().child_edit(ChildChange::Reorder {
            block: first,
            after: Some(third),
        }),
    );
    assert_eq!(moved.root().blocks(), vec![second, third, first]);
    assert_eq!(moved.references(), vec![second, third, first]);

    let moved = edited(
        &moved,
        moved.root().child_edit(ChildChange::Reorder {
            block: third,
            after: None,
        }),
    );
    assert_eq!(moved.root().blocks(), vec![third, second, first]);
}
