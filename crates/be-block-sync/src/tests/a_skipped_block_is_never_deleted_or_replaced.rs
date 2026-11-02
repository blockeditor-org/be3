use super::*;

#[tokio::test]
async fn a_skipped_block_is_never_deleted_or_replaced() {
    let harness = Harness::start().await;
    let peer = harness.peer().await;
    let code = workspace(&peer).await;
    let lower = text_block(&peer, code, "notes", "lower").await;
    let upper = text_block(&peer, code, "Notes", "upper").await;
    let device = text_block(&peer, code, "aux.txt", "a device name").await;
    let checkout = harness.checkout("checkout");
    sync(&peer, &checkout, code).await;
    assert!(!checkout.join("notes").exists());
    sync(&peer, &checkout, code).await;
    assert_eq!(children(&peer, code).await.len(), 3);

    write(&checkout, "notes", "made here");
    let report = sync(&peer, &checkout, code).await;
    assert_eq!(report.created, 1, "{report}");
    assert_eq!(text_of(&peer, lower).await, "lower");
    assert_eq!(text_of(&peer, upper).await, "upper");
    assert_eq!(text_of(&peer, device).await, "a device name");
    let listed = children(&peer, code).await;
    assert_eq!(listed.len(), 4, "{listed:?}");
    let made = listed
        .iter()
        .find(|listed| ![lower, upper, device].contains(&listed.id))
        .unwrap();
    assert_eq!(text_of(&peer, made.id).await, "made here");
    assert_eq!(read(&checkout, "notes"), "made here");

    let errors = read(&checkout, crate::ERRORS_FILE);
    assert!(errors.contains(&upper.to_string()), "{errors}");
    assert!(errors.contains(&lower.to_string()), "{errors}");
    assert!(errors.contains(&device.to_string()), "{errors}");
    let again = sync(&peer, &checkout, code).await;
    assert!(!again.changed(), "{again}");
    assert_eq!(children(&peer, code).await.len(), 4);

    peer.define(upper, |metadata| {
        metadata.insert(&be_block::NAME, &"Upper".to_owned());
    })
    .await
    .unwrap();
    sync(&peer, &checkout, code).await;
    assert_eq!(read(&checkout, "Upper"), "upper");
    harness.stop().await;
}
