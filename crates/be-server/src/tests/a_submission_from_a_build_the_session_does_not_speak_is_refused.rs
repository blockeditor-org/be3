use super::*;

/// The session speaks its first participant's build: the server refuses a submission from a
/// participant running another, so two builds of a type never edit one session.
#[tokio::test]
async fn a_submission_from_a_build_the_session_does_not_speak_is_refused() {
    let harness = Harness::start().await;
    let mut phone = harness.client().await;
    phone.register("phone@example.com").await;
    let organization = phone.organization("notes").await;
    let block = phone.create_block(BlockParent::Root).await;
    phone.join_session(block).await;
    let token = phone.token.clone();
    let mut laptop = harness.second_connection(&token, organization).await;
    let (_, state) = laptop
        .try_join_as(block, Hash::from_bytes([9; 32]))
        .await
        .unwrap();
    assert_eq!(state.tag, Some(TYPE_BUILD));

    let refused = laptop.submit(block, 2, 1, 0, b"another build's".to_vec()).await;
    assert!(refused.is_err(), "{refused:?}");
    phone.submit(block, 1, 1, 0, b"this build's".to_vec()).await.unwrap();
    assert_eq!(phone.sequenced(block).await.sequence, 1);
}
