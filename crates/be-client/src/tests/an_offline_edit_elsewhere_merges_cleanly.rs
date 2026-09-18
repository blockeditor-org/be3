use super::*;

use be_block::TextContent;
use be_commit::MergeResult;

#[tokio::test]
async fn an_offline_edit_elsewhere_merges_cleanly() {
    let harness = Harness::start().await;
    let author = harness.owner("traveller@example.com").await;
    let block = author
        .create::<TextContent>(BlockParent::Root)
        .await
        .unwrap();

    let base = "title\n\nfirst paragraph\n\nsecond paragraph\n";
    let shared = author
        .save(block, &TextContent::from(base), None)
        .await
        .unwrap()
        .published()
        .unwrap();

    let online = harness.shared(&author).await;
    online
        .save(
            block,
            &TextContent::from(
                base.replace("second paragraph", "second paragraph, revised")
                    .as_str(),
            ),
            Some(shared),
        )
        .await
        .unwrap()
        .published()
        .unwrap();

    let offline = Arc::new(author);
    let ours = offline
        .commits()
        .write(
            TextContent::CONTENT_TYPE,
            TextContent::from(
                base.replace("first paragraph", "first paragraph, expanded")
                    .as_str(),
            )
            .encode()
            .as_slice(),
            offline.account(),
            2_000,
            Some(shared),
            Vec::new(),
        )
        .unwrap();

    let mut session = Live::<_, TextContent>::join(Arc::clone(&offline), block)
        .await
        .unwrap();
    session.set_head(Some(ours));
    let outcome = session.reconcile().await.unwrap();
    assert!(
        matches!(outcome, MergeResult::Clean(())),
        "edits to separate paragraphs conflicted"
    );

    let merged = session.content().text();
    assert!(merged.contains("first paragraph, expanded"), "{merged}");
    assert!(merged.contains("second paragraph, revised"), "{merged}");
    assert!(!merged.contains("<<<<<<<"), "{merged}");

    assert_eq!(
        online
            .open::<TextContent>(block)
            .await
            .unwrap()
            .unwrap()
            .text(),
        merged
    );
    let history = online.history(block).await.unwrap();
    assert_eq!(history.len(), 3);

    harness.stop().await;
}
