use super::*;

#[tokio::test]
async fn a_record_found_forged_is_not_asked_about_again() {
    let harness = Harness::start().await;
    let owner = harness.owner("owner@example.com").await;
    let guest = harness.member(&owner, "guest@example.com").await;
    let other = harness.member(&owner, "other@example.com").await;
    let block = Uuid::new_v4();
    // `other` is no contact of the guest's, so it takes the guest's key from the server.
    let liar = be_block_keys::RecoveryPhrase::generate().secret();
    let forged = liar.sign_authored(block.into_bytes(), b"hello");
    let verify = async |bytes: &[u8], signature: &[u8]| {
        other
            .verify_authored(guest.account(), block, bytes, signature)
            .await
            .unwrap()
    };
    assert_eq!(verify(b"hello", &forged).await, Signer::Bad);

    // The server now names the forger's key as the guest's. Asked again, the record found
    // forged is answered from what this device learned, without the server; a record it never
    // asked about is checked against the key the server names now.
    harness
        .database()
        .execute(
            "UPDATE recovery_keys SET signing = ?1 WHERE account_id = ?2",
            rusqlite::params![liar.signing().0.to_vec(), guest.account().to_string()],
        )
        .unwrap();
    assert_eq!(
        verify(b"hello", &forged).await,
        Signer::Bad,
        "a forged row shown again costs no round trip"
    );
    let new = liar.sign_authored(block.into_bytes(), b"hullo");
    assert_eq!(
        verify(b"hullo", &new).await,
        Signer::Changed,
        "a record not asked about before is asked about"
    );
    harness.stop().await;
}
