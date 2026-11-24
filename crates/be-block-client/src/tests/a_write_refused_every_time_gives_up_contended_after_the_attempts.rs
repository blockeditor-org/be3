use std::sync::atomic::{AtomicUsize, Ordering};

use super::*;

/// Another device writes the block between each of the update's reads and its write: every
/// write is refused, and the update gives up with `Contended` after `UPDATE_ATTEMPTS` of them.
#[tokio::test]
async fn a_write_refused_every_time_gives_up_contended_after_the_attempts() {
    let harness = Harness::start().await;
    let author = harness.owner("first@example.com").await;
    let other = harness.second(&author).await;
    let block = note(&author).await;
    let organization = author.organization();

    let made = AtomicUsize::new(0);
    let updated = author
        .update(block, TextContent::CONTENT_TYPE, |read: crate::BlockRead| {
            let (made, other) = (&made, &other);
            async move {
                let made = made.fetch_add(1, Ordering::SeqCst);
                other
                    .save(
                        block,
                        &TextBlock::of(&format!("theirs {made}")),
                        read.summary.head,
                    )
                    .await?
                    .published()
                    .expect("the other device's write lands");
                let ours = TextBlock::of(&format!("ours {made}"));
                Ok::<_, ClientError>(Some(crate::NewContent::of(&ours, organization)))
            }
        })
        .await;

    assert!(
        matches!(updated, Err(ClientError::Contended)),
        "the update did not give up: {updated:?}"
    );
    assert_eq!(made.into_inner(), crate::UPDATE_ATTEMPTS);
    assert_eq!(
        text(&author, block).await,
        format!("theirs {}", crate::UPDATE_ATTEMPTS - 1)
    );

    harness.stop().await;
}
