use super::*;

/// An escrow secret is an X25519 secret, never a content key: the device that minted it and
/// another that opened it hold it apart from the content keyring, where an object whose header
/// named the escrow's id would open with it.
#[tokio::test]
async fn an_escrow_secret_is_held_apart_from_content_keys() {
    let harness = Harness::start().await;
    let admin = harness.owner("admin@example.com").await;
    let escrow = admin.keys().escrow().expect("the owner made an escrow key");
    let laptop = harness.second(&admin).await;
    for (device, peer) in [
        ("the device that minted it", &admin),
        ("another device", &laptop),
    ] {
        assert!(
            peer.keys().has(escrow.id),
            "{device} holds the escrow secret"
        );
        assert!(
            !peer.keys().ring().contains(escrow.id),
            "{device} keeps it out of the content keyring"
        );
    }
}
