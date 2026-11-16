use super::*;

/// What a peer relays in a block opens only under that block's keys: one sealed under another
/// block's key, which that block's readers hold, is refused rather than heard as this block's.
#[tokio::test]
async fn a_relay_sealed_under_another_blocks_key_is_refused() {
    let harness = Harness::start().await;
    let owner = harness.owner("owner@example.com").await;
    let (open, secret) = open_and_secret(&owner).await;
    let seal = |block: Uuid| {
        owner
            .commits()
            .vault()
            .seal(owner.current_key(block).expect("a key"), b"relayed")
            .unwrap()
    };

    assert_eq!(owner.unseal(secret, &seal(secret)).unwrap(), b"relayed");
    let opened = owner.unseal(secret, &seal(open));
    assert!(
        matches!(opened, Err(ClientError::Forged(block)) if block == secret),
        "a relay sealed under the open note's key is not the secret note's: {opened:?}"
    );
}
