use super::*;

#[tokio::test]
async fn a_stale_save_is_rejected_with_the_head_to_merge_against() {
    let harness = Harness::start().await;
    let author = harness.owner("first@example.com").await;
    let other = harness.second(&author).await;
    let block = author
        .create::<ImageContent>(BlockParent::Root)
        .await
        .unwrap();

    let base = image("base.png", 2_000, 5);
    let head = author
        .save(block, &base, None)
        .await
        .unwrap()
        .published()
        .unwrap();

    let theirs = image("theirs.png", 2_000, 6);
    let moved = other
        .save(block, &theirs, Some(head))
        .await
        .unwrap()
        .published()
        .unwrap();

    let ours = image("ours.png", 2_000, 7);
    let rejected = author.save(block, &ours, Some(head)).await.unwrap();
    assert_eq!(rejected, Saved::Rejected { head: Some(moved) });
    assert!(rejected.published().is_none());

    let current = author.remote_head(block).await.unwrap();
    assert_eq!(current, Some(moved));
    let republished = author.save(block, &ours, current).await.unwrap();
    assert!(matches!(republished, Saved::Published(_)));
    assert_eq!(
        other.open::<ImageContent>(block).await.unwrap().unwrap(),
        ours
    );

    harness.stop().await;
}
