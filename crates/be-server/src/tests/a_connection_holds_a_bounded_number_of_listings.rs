use super::*;
use crate::limits::*;

#[tokio::test]
async fn a_connection_holds_a_bounded_number_of_listings() {
    let hub = WatchHub::new();
    let client = 1;
    let first = Listing::Backrefs(Uuid::new_v4());
    hub.list(client, first).await.unwrap();
    for _ in 1..MAX_LISTINGS_PER_CONNECTION {
        hub.list(client, Listing::Backrefs(Uuid::new_v4()))
            .await
            .unwrap();
    }
    let listing = Listing::Children(BlockParent::Root);
    let refused = hub.list(client, listing).await;
    assert!(
        matches!(refused, Err(ServerError::Refused(ErrorCode::OverQuota, _))),
        "{refused:?}"
    );
    hub.list(client, first)
        .await
        .expect("listing again what is listed takes no room");
    let read = Listing::Block(Uuid::new_v4());
    hub.list(client, read)
        .await
        .expect("a block's own listing takes no room");
    hub.listed(client, read, &[]).await;
    hub.unlist(client, first).await;
    hub.list(client, listing)
        .await
        .expect("unlisting made room");
}
