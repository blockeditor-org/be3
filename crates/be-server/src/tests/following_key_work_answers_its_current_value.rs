use super::*;

#[tokio::test]
async fn following_key_work_answers_its_current_value() {
    let harness = Harness::start().await;
    let mut admin = harness.member("admin@example.com").await;
    admin.organization("team").await;
    let first = admin
        .call(|peer| peer.ask::<be_block_protocol::KeyWork>(Query::KeyWork, true))
        .await
        .expect("following the key work failed");
    let later = admin.key_work().await.expect("reading the key work failed");
    assert_eq!(format!("{first:?}"), format!("{later:?}"));
    assert!(
        first.escrows.is_empty() && first.rotations.is_empty(),
        "a new company has no escrow key and nothing to rotate: {first:?}"
    );

    harness.stop().await;
}
