use super::*;

/// A head of a block's signed log another account's device signed is compared only under the
/// key this device verified for that account: one only the server named is no vouching, since
/// the server could name its own, so the head is `Forged`; once a contact link verifies the
/// key, the same head is compared.
#[tokio::test]
async fn a_contacts_log_head_is_taken_only_under_its_verified_key() {
    let harness = Harness::start().await;
    let owner = harness.owner("owner@example.com").await;
    let ada = harness.member(&owner, "ada@example.com").await;
    let bo = harness.member(&owner, "bo@example.com").await;
    let chat = workspace(&ada, "Chat").await;
    ada.append(
        chat,
        "test.app",
        b"one",
        be_block_content::Along::default(),
    )
    .await
    .unwrap();
    let kept = ada.read_log(chat, 0, 10).await.unwrap().head;
    let from_bo = SignedLogHead {
        account: bo.account(),
        head: kept,
        signature: bo.keys().identity().sign_log_head(
            ada.organization().into_bytes(),
            chat.into_bytes(),
            kept.size,
            kept.root.as_bytes(),
        ),
    };

    // Ada pins Bo's key as the server names it, verifying something Bo signed.
    let said = b"hello";
    let signature = bo.sign_authored(chat, said);
    assert_eq!(
        ada.verify_authored(bo.account(), chat, said, &signature, None)
            .await
            .unwrap(),
        be_block_content::Signer::Unverified
    );
    assert!(
        matches!(
            ada.compare_log_head(chat, from_bo.clone(), HeadVia::Session).await,
            Err(ClientError::Forged(block)) if block == chat
        ),
        "a head signed with a key only the server named was compared"
    );

    befriend(&ada, &bo).await;
    assert_eq!(
        ada.compare_log_head(chat, from_bo, HeadVia::Session)
            .await
            .unwrap(),
        LogAgreement::Agrees,
        "a contact's head under the key its card verified was refused"
    );
    assert_eq!(ada.log_fork(chat), None);
    harness.stop().await;
}
