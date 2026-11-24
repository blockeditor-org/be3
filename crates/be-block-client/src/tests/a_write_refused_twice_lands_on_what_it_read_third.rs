use std::sync::Mutex;

use super::*;

/// Another device writes the block between the update's first two reads and their writes. Each
/// refused write is made again from the block as read next, and the third lands, made from the
/// other device's last write.
#[tokio::test]
async fn a_write_refused_twice_lands_on_what_it_read_third() {
    let harness = Harness::start().await;
    let author = harness.owner("first@example.com").await;
    let other = harness.second(&author).await;
    let block = note(&author).await;
    // Its writes are refused on purpose: on a manual clock its backoff waits for no real time.
    harness.on_manual_clock(&author).await;
    let organization = author.organization();

    let reads: Mutex<Vec<String>> = Mutex::new(Vec::new());
    let saved = author
        .update(block, TextContent::CONTENT_TYPE, |read: crate::BlockRead| {
            let (reads, other) = (&reads, &other);
            async move {
                let held = match &read.stored {
                    Some(stored) => TextBlock::text(&<TextContent as BlockContent>::from_stored(stored)?),
                    None => String::new(),
                };
                let made = {
                    let mut reads = reads.lock().unwrap();
                    reads.push(held.clone());
                    reads.len()
                };
                if made <= 2 {
                    other
                        .save(
                            block,
                            &TextBlock::of(&format!("theirs {made}")),
                            read.summary.head,
                        )
                        .await?
                        .published()
                        .expect("the other device's write lands");
                }
                let ours = TextBlock::of(&format!("{held}, then ours"));
                Ok::<_, ClientError>(Some(crate::NewContent::of(&ours, organization)))
            }
        })
        .await
        .unwrap();

    assert!(matches!(saved, Some(Saved::Published(_))), "{saved:?}");
    assert_eq!(reads.into_inner().unwrap(), ["", "theirs 1", "theirs 2"]);
    assert_eq!(text(&other, block).await, "theirs 2, then ours");

    harness.stop().await;
}
