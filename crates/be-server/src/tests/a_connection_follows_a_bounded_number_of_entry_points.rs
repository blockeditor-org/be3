use super::*;
use crate::limits::*;

#[tokio::test]
async fn a_connection_follows_a_bounded_number_of_entry_points() {
    let hub = WatchHub::new();
    let client = 1;
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
    let first = Uuid::new_v4();
    hub.follow(client, first).await.unwrap();
    for _ in 1..MAX_FOLLOWS_PER_CONNECTION {
        hub.follow(client, Uuid::new_v4()).await.unwrap();
    }
    let entry = Uuid::new_v4();
    let refused = hub.follow(client, entry).await;
    assert!(
        matches!(refused, Err(ServerError::Refused(ErrorCode::OverQuota, _))),
        "{refused:?}"
    );
    hub.follow(client, first)
        .await
        .expect("following again what is followed takes no room");
    hub.unfollow(client, first).await;
    hub.follow(client, entry)
        .await
        .expect("unfollowing made room");
}
