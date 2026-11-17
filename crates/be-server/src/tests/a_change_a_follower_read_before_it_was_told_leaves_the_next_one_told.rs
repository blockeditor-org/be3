use super::*;

/// A change's write reaches the hub after it commits, so a follower's read can take it first.
/// Told of it then, the follower has nothing to read; the next change is told all the same.
#[tokio::test]
async fn a_change_a_follower_read_before_it_was_told_leaves_the_next_one_told() {
    let hub = WatchHub::new();
    let (sender, mut pushes) = tokio::sync::mpsc::channel(8);
    let client = hub.register(sender).await;
    hub.join_organization(
        client,
        Identity {
            account: Uuid::new_v4(),
            organization: Uuid::new_v4(),
            role: OrganizationRole::Editor,
            bound: None,
        },
    )
    .await;
    let entry = Uuid::new_v4();
    hub.follow(client, entry).await.unwrap();
    // The read saw the change at 17 before its write told the hub.
    hub.read(client, entry, 17).await;
    hub.advance(client, entry, 17).await;
    assert!(
        pushes.try_recv().is_err(),
        "a change the follower read is not told"
    );
    hub.advance(client, entry, 18).await;
    assert!(
        matches!(
            pushes.try_recv(),
            Ok(Push::TreeAdvanced { entry: told, sequence: 18 }) if told == entry
        ),
        "the next change is told"
    );
}
