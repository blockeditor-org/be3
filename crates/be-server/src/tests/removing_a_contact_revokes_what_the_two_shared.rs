use super::*;
use be_graph::Access;

#[tokio::test]
async fn removing_a_contact_revokes_what_the_two_shared() {
    let harness = Harness::start().await;
    let mut owner = harness.client().await;
    let owner_id = owner.register("owner@example.com").await;
    owner.personal().await;
    let mut other = harness.client().await;
    let other_id = other.register("other@example.com").await;
    other.personal().await;
    let mine = owner.create_block(BlockParent::Root).await;
    let theirs = other.create_block(BlockParent::Root).await;

    let link = owner.link(None).await;
    let _ = other.accept(link, &[]).await;
    for (granter, block, grantee) in [(&mut owner, mine, other_id), (&mut other, theirs, owner_id)]
    {
        let granted = granter.grant(block, grantee, Access::Edit).await;
        assert!(matches!(granted, Ok(())), "{granted:?}");
    }
    assert_eq!(other.read_block(mine).await.access, Access::Edit);
    assert_eq!(owner.read_block(theirs).await.access, Access::Edit);

    let removed = owner
        .call(|peer| client::contacts::remove_contact(peer, other_id))
        .await;
    assert!(matches!(removed, Ok(())), "{removed:?}");
    assert!(owner.contacts().await.is_empty());
    assert!(other.contacts().await.is_empty());
    for (client, block) in [(&mut other, mine), (&mut owner, theirs)] {
        let read = client
            .call(|peer| client::blocks::read_block(peer, block))
            .await;
        assert!(
            matches!(read, Err(_)),
            "a grant outlived the contact: {read:?}"
        );
    }
    let again = owner.grant(mine, other_id, Access::View).await;
    assert!(refused_as(&again, ErrorCode::NotAContact), "{again:?}");

    harness.stop().await;
}
