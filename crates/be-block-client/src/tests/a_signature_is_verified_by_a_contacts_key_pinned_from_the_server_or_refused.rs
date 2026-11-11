use super::*;

#[tokio::test]
async fn a_signature_is_verified_by_a_contacts_key_pinned_from_the_server_or_refused() {
    let harness = Harness::start().await;
    let owner = harness.owner("owner@example.com").await;
    let guest = harness.member(&owner, "guest@example.com").await;
    let other = harness.member(&owner, "other@example.com").await;
    let block = Uuid::new_v4();
    let signed = guest.sign_authored(block, b"hello");
    let forged = other.sign_authored(block, b"hello");

    let account = guest.account();
    let verify = async |peer: &Peer<MemoryStore>, bytes: &[u8], signature: Vec<u8>| {
        peer.verify_authored(account, block, bytes, &signature)
            .await
            .unwrap()
    };
    assert_eq!(
        verify(&guest, b"hello", signed.clone()).await,
        Signer::Verified,
        "an account verifies its own signature"
    );
    assert_eq!(
        verify(&owner, b"hello", signed.clone()).await,
        Signer::Verified,
        "the owner verified the guest's card through the invitation"
    );
    assert_eq!(
        verify(&owner, b"hello", forged.clone()).await,
        Signer::Bad,
        "another member cannot sign as the guest"
    );
    assert_eq!(verify(&owner, b"hullo", signed.clone()).await, Signer::Bad);
    assert_eq!(
        owner
            .verify_authored(guest.account(), Uuid::new_v4(), b"hello", &signed)
            .await
            .unwrap(),
        Signer::Bad,
        "a signature made in one block does not verify in another"
    );

    assert_eq!(
        verify(&other, b"hello", signed.clone()).await,
        Signer::Unverified,
        "a member who is not a contact takes the guest's key from the server"
    );
    assert_eq!(verify(&other, b"hello", forged.clone()).await, Signer::Bad);

    // The server now names a key of its choosing as the guest's.
    let liar = be_block_keys::RecoveryPhrase::generate().secret();
    harness
        .database()
        .execute(
            "UPDATE recovery_keys SET signing = ?1 WHERE account_id = ?2",
            rusqlite::params![liar.signing().0.to_vec(), guest.account().to_string()],
        )
        .unwrap();
    let lie = liar.sign_authored(block.into_bytes(), b"hello");
    assert_eq!(
        verify(&other, b"hello", lie.clone()).await,
        Signer::Changed,
        "a member who pinned the guest's key is warned that it changed"
    );
    assert_eq!(
        verify(&other, b"hello", signed.clone()).await,
        Signer::Unverified,
        "what the guest signed still verifies with the key pinned"
    );
    assert_eq!(
        verify(&owner, b"hello", lie).await,
        Signer::Bad,
        "the server cannot sign as a contact"
    );

    harness.stop().await;
}
