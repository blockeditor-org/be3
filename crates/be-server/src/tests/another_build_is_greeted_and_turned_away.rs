use super::*;

#[tokio::test]
async fn another_build_is_greeted_and_turned_away() {
    let harness = Harness::start().await;

    let mut stranger = harness.greeted("another build").await;
    assert_eq!(
        next_binary(&mut stranger).await.as_deref().and_then(read_hello).as_deref(),
        Some(BUILD),
        "the server did not say which build it is"
    );
    assert_eq!(
        next_binary(&mut stranger).await,
        None,
        "the server kept talking to another build"
    );

    let mut unversioned = harness.greeted(build_id::UNKNOWN).await;
    assert_eq!(
        next_binary(&mut unversioned).await.as_deref().and_then(read_hello).as_deref(),
        Some(BUILD)
    );
    unversioned
        .send(WsMessage::Binary(
            encode(&ClientMessage::ListWorkspaces { request: 1 }).unwrap(),
        ))
        .await
        .unwrap();
    assert!(
        next_binary(&mut unversioned).await.is_some(),
        "a build without an id was not served"
    );

    harness.stop().await;
}
