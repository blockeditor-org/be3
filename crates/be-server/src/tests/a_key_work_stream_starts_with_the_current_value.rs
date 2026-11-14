use super::*;

#[tokio::test]
async fn a_key_work_stream_starts_with_the_current_value() {
    let harness = Harness::start().await;
    let mut admin = harness.client().await;
    admin.register("admin@example.com").await;
    // Opened before any organization is: it waits for one, then starts with its key work.
    let mut early = client::keys::key_work_changed(&admin.peer);
    admin.organization("team").await;
    let first = early
        .recv()
        .await
        .expect("the stream starts once an organization is open");
    let later = admin
        .key_work()
        .await
        .expect("a stream opened now starts too");
    assert_eq!(format!("{first:?}"), format!("{later:?}"));
    assert!(
        first.escrows.is_empty() && first.rotations.is_empty(),
        "a new company has no escrow key and nothing to rotate: {first:?}"
    );

    harness.stop().await;
}
