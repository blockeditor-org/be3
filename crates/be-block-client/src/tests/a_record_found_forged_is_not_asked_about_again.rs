use super::*;

#[tokio::test]
async fn a_record_found_forged_is_not_asked_about_again() {
    let harness = Harness::start().await;
    let owner = harness.owner("owner@example.com").await;
    let guest = harness.member(&owner, "guest@example.com").await;
    let other = harness.member(&owner, "other@example.com").await;
    let block = Uuid::new_v4();
    // `other` is no contact of the guest's, so it takes the guest's key from the server.
    let forged = owner.sign_authored(block, b"hello");
    let also_forged = owner.sign_authored(block, b"hullo");
    assert_eq!(
        other
            .verify_authored(guest.account(), block, b"hello", &forged)
            .await
            .unwrap(),
        Signer::Bad,
        "the owner cannot sign as the guest"
    );

    // With the server gone, the record found forged is answered from what this device learned,
    // while one it never asked about needs the server.
    harness.stop().await;
    assert_eq!(
        other
            .verify_authored(guest.account(), block, b"hello", &forged)
            .await
            .unwrap(),
        Signer::Bad,
        "a forged row shown again costs no round trip"
    );
    assert!(
        other
            .verify_authored(guest.account(), block, b"hullo", &also_forged)
            .await
            .is_err(),
        "a record not asked about before is not taken as forged without the server"
    );
}
