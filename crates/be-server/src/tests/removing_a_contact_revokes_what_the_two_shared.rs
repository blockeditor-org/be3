use super::*;
use be_block_graph::Access;

#[tokio::test]
async fn removing_a_contact_revokes_what_the_two_shared() {
    let harness = Harness::start().await;
    let mut owner = harness.member("owner@example.com").await;
    let owner_id = owner.id();
    owner.personal().await;
    let mut other = harness.member("other@example.com").await;
    let other_id = other.id();
    other.personal().await;
    let mine = owner.create_block(BlockParent::Root).await;
    let theirs = other.create_block(BlockParent::Root).await;

    let link = owner.link().await;
    let _ = other.accept(link, &[]).await;
    for (granter, block, grantee) in [(&mut owner, mine, other_id), (&mut other, theirs, owner_id)]
    {
        let granted = granter.grant(block, grantee, Access::Edit).await;
        assert!(matches!(granted, Ok(())), "{granted:?}");
    }
    assert_eq!(other.read_block(mine).await.access, Access::Edit);
    assert_eq!(owner.read_block(theirs).await.access, Access::Edit);

    let removed = owner.remove_contact(other_id).await;
    assert!(matches!(removed, Ok(())), "{removed:?}");
    assert!(owner.contacts().await.is_empty());
    assert!(other.contacts().await.is_empty());
    for (client, block) in [(&mut other, mine), (&mut owner, theirs)] {
        let read = client.try_read_block(block).await;
        assert!(
            read.is_err(),
            "a grant outlived the contact: {read:?}"
        );
    }
    let again = owner.grant(mine, other_id, Access::View).await;
    assert!(refused_as(&again, ErrorCode::NotAContact), "{again:?}");

    harness.stop().await;
}
