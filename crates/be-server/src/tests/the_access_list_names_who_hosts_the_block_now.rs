use super::*;
use be_block_graph::Access;

async fn listed_host(client: &mut TestClient, block: Uuid) -> Option<Uuid> {
    let response = client
        .call(|peer| {
            peer.read::<be_block_protocol::AccessList>(be_block_protocol::Query::Access(block))
        })
        .await;
    let Ok(list) = response else {
        panic!("listing access failed: {response:?}");
    };
    list.host
}

/// The access list names who hosts the block now (plans/hosted_blocks.md, "The share dialog"):
/// the account whose connection holds its live session's lease, never a viewer, and no one once
/// the session is empty.
#[tokio::test]
async fn the_access_list_names_who_hosts_the_block_now() {
    let harness = Harness::start().await;
    let mut owner = harness.member("owner@example.com").await;
    let owner_account = owner.id();
    owner.personal().await;
    let mut viewer = harness.member("viewer@example.com").await;
    let viewer_account = viewer.id();
    viewer.personal().await;
    let link = owner.link(None).await;
    let _ = viewer.accept(link, &[]).await;
    let block = owner.create_block(BlockParent::Root).await;
    let granted = owner.grant(block, viewer_account, Access::View).await;
    assert!(matches!(granted, Ok(())), "{granted:?}");

    assert_eq!(listed_host(&mut owner, block).await, None, "no session yet");
    let _ = viewer.join_session(block).await;
    assert_eq!(
        listed_host(&mut owner, block).await,
        None,
        "a viewer in the session hosts nothing"
    );
    let _ = owner.join_session(block).await;
    assert_eq!(
        listed_host(&mut owner, block).await,
        Some(owner_account),
        "the editor holding the lease hosts it"
    );
    owner.leave(block).await;
    assert_eq!(
        listed_host(&mut owner, block).await,
        None,
        "no one hosts once the editor left"
    );

    harness.stop().await;
}
