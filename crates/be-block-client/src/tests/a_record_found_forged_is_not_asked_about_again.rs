use super::*;

#[tokio::test]
async fn a_record_found_forged_is_not_asked_about_again() {
    let harness = Harness::start().await;
    let owner = harness.owner("owner@example.com").await;
    let guest = harness.member(&owner, "guest@example.com").await;
    let other = harness.member(&owner, "other@example.com").await;
    let block = Uuid::new_v4();
    let forged = other.sign_authored(block, b"hello");
    let also_forged = other.sign_authored(block, b"hullo");
    assert_eq!(
        owner
            .verify_authored(guest.account(), block, b"hello", &forged)
            .await
            .unwrap(),
        Signer::Bad,
        "another member cannot sign as the guest"
    );

    // With the server gone, the record found forged is answered from what this device learned,
    // while one it never asked about needs the server.
    harness.stop().await;
    assert_eq!(
        owner
            .verify_authored(guest.account(), block, b"hello", &forged)
            .await
            .unwrap(),
        Signer::Bad,
        "a forged row shown again costs no round trip"
    );
    assert!(
        owner
            .verify_authored(guest.account(), block, b"hullo", &also_forged)
            .await
            .is_err(),
        "a record not asked about before is not taken as forged without the server"
    );
}
