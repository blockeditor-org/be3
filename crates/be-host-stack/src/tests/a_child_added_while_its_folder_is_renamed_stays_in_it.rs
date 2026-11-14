use super::*;
use be_block_content::ChildChange;
use be_type_workspace::WorkspaceContent;

/// A definition is written beside the folder's open replica, which still holds the new child
/// unsealed, and waits in the outbox: the replica takes the definition's commit in and seals the
/// child on top of it, rather than beside it, where the server would refuse one of the two.
#[test]
fn a_child_added_while_its_folder_is_renamed_stays_in_it() {
    let harness = Harness::start();
    harness.connect();
    wait_until("knew the workspace's type", |_| {
        is_known(WorkspaceContent::CONTENT_TYPE)
    });
    let (workspace, child) = (Uuid::new_v4(), Uuid::new_v4());
    create(
        workspace,
        WorkspaceContent::CONTENT_TYPE,
        be_block_graph::BlockParent::Root,
        be_block_content::BlockMetadata::named("Team"),
        None,
        false,
    );
    create(
        child,
        CounterContent::CONTENT_TYPE,
        be_block_graph::BlockParent::Block(workspace),
        be_block_content::BlockMetadata::default(),
        None,
        false,
    );
    change_child(
        workspace,
        WorkspaceContent::CONTENT_TYPE,
        ChildChange::Add(child),
    );
    set_name(workspace, Some("Team two".to_owned()));
    wait_until("everything reached the server", |shared| {
        shared.busy(true).is_empty() && shared.unsynced().is_empty()
    });
    let held = runtime().block_on(async {
        let peer = harness.outside_peer().await;
        let head = peer
            .remote_head(workspace)
            .await
            .expect("the workspace's head is read")
            .expect("the workspace has a head");
        let commit = peer
            .load_commit(head)
            .await
            .expect("the workspace's head is read");
        let metadata = peer
            .metadata_of(head)
            .await
            .expect("the workspace's metadata is read");
        (commit.references, metadata.get(&be_block_content::NAME))
    });
    let (references, name) = held;
    assert!(
        references.contains(&child) && name.as_deref() == Some("Team two"),
        "the server's head of the workspace holds both its new child and its new name, not \
         {references:?} and {name:?}"
    );
}
