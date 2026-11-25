use super::a_graph_change_reaches_only_the_members_who_may_see_it::join;
use super::*;
use be_block_graph::Access;

#[tokio::test]
async fn removing_a_member_changes_every_block_they_could_view() {
    let harness = Harness::start().await;
    let mut owner = harness.member("owner@example.com").await;
    let organization = owner.organization("shared").await;
    let (mut author, author_account) =
        join(&harness, &mut owner, organization, "author@example.com", 1).await;
    let owner_account = owner.id();

    // The author makes a block and shares it with the owner, its contact by the invitation,
    // and makes another below it: the author holds no grant on either, viewing both as their
    // author.
    let shared = author.create_block(BlockParent::Root).await;
    let below = author.create_block(BlockParent::Block(shared)).await;
    let response = author.grant(shared, owner_account, Access::View).await;
    assert!(matches!(response, Ok(())), "{response:?}");
    let entry = owner.follow(below).await;
    assert_eq!(entry, shared);
    let before = [
        owner.read_block(shared).await.changed,
        owner.read_block(below).await.changed,
    ];

    let response = owner.remove_member(author_account).await;
    assert!(matches!(response, Ok(())), "{response:?}");
    // Who may view each changed, so each block's change moves, and the owner following
    // them is told their tree advanced.
    let advanced = async {
        loop {
            if let Notice::TreeAdvanced { entry: told, .. } = owner.graph_change().await
                && told == entry
            {
                return;
            }
        }
    };
    tokio::time::timeout(std::time::Duration::from_secs(10), advanced)
        .await
        .expect("the owner is told the tree advanced");
    let after = [
        owner.read_block(shared).await.changed,
        owner.read_block(below).await.changed,
    ];
    assert!(
        after[0] > before[0] && after[1] > before[1],
        "{before:?} then {after:?}"
    );
}
