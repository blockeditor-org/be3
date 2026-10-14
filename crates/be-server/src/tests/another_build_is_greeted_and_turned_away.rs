use super::*;

#[tokio::test]
async fn another_build_is_greeted_and_turned_away() {
    let harness = Harness::start().await;

    let mut stranger = harness.greeted("another build").await;
    assert_eq!(
        next_binary(&mut stranger)
            .await
            .as_deref()
            .and_then(read_hello)
            .as_deref(),
        Some(BUILD),
        "the server did not say which build it is"
    );
    assert_eq!(
        next_binary(&mut stranger).await,
        None,
        "the server kept talking to another build"
    );

    let mut unversioned = harness.greeted("unknown").await;
    assert_eq!(
        next_binary(&mut unversioned)
            .await
            .as_deref()
            .and_then(read_hello)
            .as_deref(),
        Some(BUILD)
    );
    assert_eq!(
        next_binary(&mut unversioned).await,
        None,
        "the server talked to a build that does not know its id"
    );

    harness.stop().await;
}
