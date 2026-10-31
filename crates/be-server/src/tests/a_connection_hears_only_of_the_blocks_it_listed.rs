use super::*;
use be_protocol::Listing;

async fn rename(client: &mut TestClient, block: Uuid, name: &[u8]) {
    let response = client
        .call(|peer| client::blocks::set_metadata(peer, block, name.to_vec(), Vec::new()))
        .await;
    assert!(matches!(response, Ok(_)), "{response:?}");
}

async fn heard(client: &mut TestClient) -> Uuid {
    match client.graph_change().await {
        Notice::BlockChanged { block } => block.id,
        Notice::BlockRemoved { block, .. } => block,
        other => panic!("{other:?}"),
    }
}

#[tokio::test]
async fn a_connection_hears_only_of_the_blocks_it_listed() {
    let harness = Harness::start().await;
    let mut author = harness.client().await;
    author.register("author@example.com").await;
    let organization = author.organization("notes").await;
    let mut other = harness
        .second_connection(&author.token.clone(), organization)
        .await;
    other.list(BlockParent::Root).await;
    let listed = author.create_block(BlockParent::Root).await;
    assert_eq!(heard(&mut other).await, listed);
    let closed = author.create_block(BlockParent::Root).await;
    assert_eq!(heard(&mut other).await, closed);

    author.create_block(BlockParent::Block(closed)).await;
    rename(&mut author, listed, b"after").await;
    assert_eq!(
        heard(&mut other).await,
        listed,
        "a block made in a folder nobody listed is not announced"
    );

    other.list(BlockParent::Block(listed)).await;
    let inside = author.create_block(BlockParent::Block(listed)).await;
    assert_eq!(heard(&mut other).await, inside);
    let response = author
        .call(|peer| client::blocks::delete_block(peer, inside))
        .await;
    assert!(matches!(response, Ok(())), "{response:?}");
    assert_eq!(
        heard(&mut other).await,
        inside,
        "a block leaving a listed folder is announced"
    );

    let response = other
        .call(|peer| client::blocks::unlist(peer, Listing::Children(BlockParent::Root)))
        .await;
    assert!(matches!(response, Ok(())), "{response:?}");
    rename(&mut author, closed, b"unheard").await;
    let last = author.create_block(BlockParent::Block(listed)).await;
    assert_eq!(
        heard(&mut other).await,
        last,
        "nothing in a folder is announced once it is unlisted"
    );

    harness.stop().await;
}
