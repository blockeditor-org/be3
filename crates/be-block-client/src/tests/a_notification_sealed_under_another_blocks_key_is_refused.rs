use super::*;

use be_block_protocol::PushMessage;

/// A notification about a block opens only under that block's keys: one a reader of another
/// block sealed under its key, naming this block, is refused.
#[tokio::test]
async fn a_notification_sealed_under_another_blocks_key_is_refused() {
    let harness = Harness::start().await;
    let owner = harness.owner("owner@example.com").await;
    let (open, secret) = open_and_secret(&owner).await;
    let sent = 1_000_000;
    let pushed = |key: Uuid| PushMessage {
        organization: Uuid::nil(),
        block: secret,
        sender: owner.account(),
        sealed: owner
            .commits()
            .vault()
            .seal(
                key,
                &crate::Notification {
                    organization: Uuid::nil(),
                    block: secret,
                    sender: owner.account(),
                    sent,
                    text: "Lunch?".into(),
                }
                .encode(),
            )
            .unwrap(),
    };

    let secret_key = owner.current_key(secret).expect("a key");
    assert!(owner.open_notification(&pushed(secret_key), sent + 1).is_ok());
    let open_key = owner.current_key(open).expect("a key");
    let opened = owner.open_notification(&pushed(open_key), sent + 1);
    assert!(
        matches!(opened, Err(ClientError::Forged(block)) if block == secret),
        "a notification sealed under the open note's key is refused: {opened:?}"
    );
}
