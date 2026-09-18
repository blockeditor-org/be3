use super::*;

use be_block::TextContent;
use be_commit::MergeResult;

#[tokio::test]
async fn an_offline_rewrite_conflicts_instead_of_interleaving() {
    let harness = Harness::start().await;
    let author = harness.owner("offline@example.com").await;
    let block = author
        .create::<TextContent>(BlockParent::Root)
        .await
        .unwrap();

    let base: String = (0..20).map(|line| format!("line {line}\n")).collect();
    let shared = author
        .save(block, &TextContent::from(base.as_str()), None)
        .await
        .unwrap()
        .published()
        .unwrap();

    let online = harness.shared(&author).await;
    let theirs = base.replace("line 7\n", "line seven, edited\n");
    online
        .save(block, &TextContent::from(theirs.as_str()), Some(shared))
        .await
        .unwrap()
        .published()
        .unwrap();

    let offline = Arc::new(author);
    let retyped: String = (0..20).map(|line| format!("rewritten {line}\n")).collect();
    let rejected = offline
        .save(block, &TextContent::from(retyped.as_str()), Some(shared))
        .await
        .unwrap();
    assert!(
        matches!(rejected, Saved::Rejected { .. }),
        "the stale save was accepted: {rejected:?}"
    );

    let ours = offline
        .commits()
        .write(
            TextContent::CONTENT_TYPE,
            TextContent::from(retyped.as_str()).encode().as_slice(),
            offline.account(),
            2_000,
            Some(shared),
            Vec::new(),
        )
        .unwrap();
    offline.remember(block, ours);

    let mut session = Live::<_, TextContent>::join(Arc::clone(&offline), block)
        .await
        .unwrap();
    session.set_head(Some(ours));
    let outcome = session.reconcile().await.unwrap();

    let MergeResult::Conflicted { conflicts, .. } = outcome else {
        panic!("a full rewrite silently absorbed a concurrent edit");
    };
    assert_eq!(conflicts, 1);

    let merged = session.content().text();
    assert!(merged.contains("<<<<<<< local"), "{merged}");
    assert!(merged.contains("rewritten 0"), "{merged}");
    assert!(merged.contains("line seven, edited"), "{merged}");

    let published = online
        .open::<TextContent>(block)
        .await
        .unwrap()
        .unwrap()
        .text();
    assert_eq!(published, merged, "the merge was not published");

    harness.stop().await;
}
