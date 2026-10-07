use super::*;

#[tokio::test]
async fn a_follower_typing_while_the_owner_types_keeps_both_runs_whole() {
    let harness = Harness::start().await;
    let first = harness.owner("first@example.com").await;
    let block = first
        .create::<NotesContent>(BlockParent::Root)
        .await
        .unwrap();
    first.save(block, &notes("base"), None).await.unwrap();
    let owner_peer = Arc::new(first);
    let follower_peer = harness.shared(&owner_peer).await;
    let mut owner = Live::<_, NotesContent>::join(Arc::clone(&owner_peer), block)
        .await
        .unwrap();
    let mut follower = Live::<_, NotesContent>::join(Arc::clone(&follower_peer), block)
        .await
        .unwrap();
    until(
        &mut [&mut owner, &mut follower],
        "loaded the note",
        |sessions| sessions.iter().all(|session| body_of(session) == "base"),
    )
    .await;

    let letters = "abcdefghijklmnopqrstuvwxyzabcdefghijklmn";
    let digits = "0123456789012345678901234567890123456789";
    for at in 0..letters.len() {
        type_into(&mut follower, at, &letters[at..=at]).await;
        let end = body_of(&owner).len();
        type_into(&mut owner, end, &digits[at..=at]).await;
    }
    let expected = format!("{letters}base{digits}");
    until(&mut [&mut owner, &mut follower], "converged", |sessions| {
        sessions.iter().all(|session| body_of(session) == expected)
    })
    .await;

    assert!(follower.is_clean());
    harness.stop().await;
}
