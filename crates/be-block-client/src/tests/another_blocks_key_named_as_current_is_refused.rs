use super::*;

use be_type_text::{TextBlock, TextContent};

/// The server names which of a block's keys is current; a key of another block, which that
/// block's readers hold, is refused rather than sealing this block where they read it.
#[tokio::test]
async fn another_blocks_key_named_as_current_is_refused() {
    let harness = Harness::start().await;
    let owner = harness.owner("owner@example.com").await;
    let shared = workspace(&owner, "Shared").await;
    let private = workspace(&owner, "Private").await;
    let open = owner
        .create::<TextContent>(BlockParent::Block(shared))
        .await
        .unwrap();
    let secret = owner
        .create::<TextContent>(BlockParent::Block(private))
        .await
        .unwrap();
    owner
        .save(open, &TextBlock::of("open"), None)
        .await
        .unwrap();
    let open_key = owner.current_key(open).expect("the open note has a key");

    // The server says the secret note's current key is the open note's.
    let mut summary = owner.summary(secret).await.unwrap();
    summary.key = Some(open_key);
    owner.note(&summary);
    let saved = owner.save(secret, &TextBlock::of("secret"), None).await;
    assert!(
        matches!(saved, Err(ClientError::Forged(block)) if block == secret),
        "the secret note is not sealed under the open note's key: {saved:?}"
    );
}
