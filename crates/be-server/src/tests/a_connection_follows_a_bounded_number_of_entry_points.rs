use super::*;
use crate::limits::*;

#[tokio::test]
async fn a_connection_follows_a_bounded_number_of_entry_points() {
    let hub = WatchHub::new();
    let client = 1;
    let organization = Uuid::new_v4();
    let place = |block| crate::watch::Place {
        organization,
        block,
    };
    hub.join_organization(
        client,
        Identity {
            account: Uuid::new_v4(),
            organization,
            role: OrganizationRole::Editor,
            bound: None,
        },
    )
    .await;
    let first = Uuid::new_v4();
    hub.follow(client, place(first)).await.unwrap();
    for _ in 1..MAX_FOLLOWS_PER_CONNECTION {
        hub.follow(client, place(Uuid::new_v4())).await.unwrap();
    }
    let entry = Uuid::new_v4();
    let refused = hub.follow(client, place(entry)).await;
    assert!(
        matches!(refused, Err(ServerError::Refused(ErrorCode::OverQuota, _))),
        "{refused:?}"
    );
    hub.follow(client, place(first))
        .await
        .expect("following again what is followed takes no room");
    hub.unfollow(client, place(first)).await;
    hub.follow(client, place(entry))
        .await
        .expect("unfollowing made room");
}
