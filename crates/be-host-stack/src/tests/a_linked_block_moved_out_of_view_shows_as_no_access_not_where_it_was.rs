use super::*;
use be_type_workspace::WorkspaceContent;

use be_block_content::block_url::block_url;
use be_type_text::{TextBlock, TextContent};

#[test]
fn a_linked_block_moved_out_of_view_shows_as_no_access_not_where_it_was() {
    let harness = Harness::start();
    let sharer = Someone::register(
        harness.server.directory(),
        &harness.url,
        "sharer@example.com",
        "Sharer",
    );
    let personal = harness.connect_personal();
    let code = made_link(None);
    let runtime = runtime();
    let peer = runtime.block_on(sharer.peer(&harness.url, personal));
    runtime
        .block_on(be_block_client::contacts::accept_link(
            &peer.connection(),
            &sharer.phrase.secret(),
            &sharer.me().card(),
            &code,
            be_block_client::contacts::Expected::Contact,
            &Default::default(),
        ))
        .expect("the sharer accepts the link");
    runtime
        .block_on(peer.refresh_contacts())
        .expect("the sharer lists its contacts");
    let public = peer
        .verified_identity(harness.account)
        .expect("the sharer verified this account through the link");

    let workspace = |name: &'static str| {
        let peer = &peer;
        async move {
            peer.create_block(
                Uuid::now_v7(),
                WorkspaceContent::CONTENT_TYPE,
                be_block_graph::BlockParent::Root,
                &be_block_content::BlockMetadata::named(name),
                false,
            )
            .await
            .unwrap()
            .id
        }
    };
    let (space, hidden, linking, linked) = runtime.block_on(async {
        let space = workspace("Shared").await;
        let hidden = workspace("Hidden").await;
        let inside = be_block_graph::BlockParent::Block(space);
        let linked = peer.create::<TextContent>(inside).await.unwrap();
        peer.save(linked, &TextBlock::of("Plan"), None)
            .await
            .unwrap();
        let linking = peer.create::<TextContent>(inside).await.unwrap();
        let url = block_url("https://example.com/", personal, linked);
        peer.save(linking, &TextBlock::of(&format!("See {url}")), None)
            .await
            .unwrap();
        peer.share(
            space,
            harness.account,
            be_block_graph::Access::View,
            &public,
        )
        .await
        .unwrap();
        (space, hidden, linking, linked)
    });

    ask_for(vec![Query::Children(space), Query::References(linking)]);
    wait_until("showed the folder and what links inside it", |held| {
        held.graph.get(linked).is_some_and(|node| {
            node.access.can_view() && node.parent == Some(be_block_graph::BlockParent::Block(space))
        }) && held
            .graph
            .get(linking)
            .is_some_and(|node| node.refers_to(linked) == Some(true))
    });

    runtime
        .block_on(peer.set_parent(linked, be_block_graph::BlockParent::Block(hidden)))
        .expect("the sharer moves the linked block somewhere hidden");
    wait_until("knew the linked block only exists, still linked", |held| {
        held.graph
            .get(linked)
            .is_some_and(|node| node.access == be_block_graph::Access::KnowExists)
    });
    let node = node(linked).expect("still shown where it is linked");
    assert_eq!(node.parent, None, "not under the folder it left");
    assert!(
        !query(Query::Children(space))
            .iter()
            .any(|node| node.id == linked),
        "the folder no longer shows it"
    );
    assert!(
        query(Query::References(linking))
            .iter()
            .any(|node| node.id == linked),
        "the block that links to it still does"
    );
}
