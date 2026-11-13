use super::*;

use be_type_text::{TextBlock, TextContent};

#[tokio::test]
async fn a_summary_read_before_a_publish_never_brings_back_the_old_head() {
    let harness = Harness::start().await;
    let owner = harness.owner("owner@example.com").await;
    let team = workspace(&owner, "Team").await;
    let note = owner
        .create::<TextContent>(BlockParent::Block(team))
        .await
        .unwrap();
    owner
        .save(note, &TextBlock::of("before"), None)
        .await
        .unwrap();
    let stale = owner.summary(note).await.unwrap();
    let before = stale.head.unwrap();

    let after = owner
        .save(note, &TextBlock::of("after"), Some(before))
        .await
        .unwrap()
        .published()
        .unwrap();
    assert_eq!(owner.known_remote_head(note), Some(after));

    // Read before the publish, answered after it.
    owner.note(&stale);
    assert_eq!(
        owner.known_remote_head(note),
        Some(after),
        "a summary read before the publish put the old head back"
    );

    harness.stop().await;
}
