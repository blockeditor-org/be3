use super::*;
use workspace_ui_types::WorkspaceContent;

use be_block::block_url::block_url;
use text_block_types::{TextBlock, TextContent};

#[test]
fn a_block_a_shared_one_links_to_shows_as_no_access_and_nowhere_else() {
    let harness = Harness::start();
    let sharer = Someone::register(
        &harness.directory.join("server"),
        &harness.url,
        "sharer@example.com",
        "Sharer",
    );
    let personal = harness.connect_personal();
    let code = made_link(None);
    let runtime = runtime();
    let peer = runtime.block_on(sharer.peer(&harness.url, personal));
    runtime
        .block_on(be_client::contacts::accept_link(
            &peer.connection(),
            &sharer.phrase.secret(),
            &sharer.me().card(),
            &code,
        ))
        .expect("the sharer accepts the link");
    runtime
        .block_on(peer.refresh_contacts())
        .expect("the sharer lists its contacts");
    let public = peer
        .keys()
        .contact(harness.account)
        .expect("the sharer verified this account through the link");

    let (shared, linked) = runtime.block_on(async {
        let space = peer
            .create_block(
                Uuid::now_v7(),
                WorkspaceContent::CONTENT_TYPE,
                be_graph::BlockParent::Root,
                &be_block::BlockMetadata::named("Sharer's"),
            )
            .await
            .unwrap()
            .id;
        let inside = be_graph::BlockParent::Block(space);
        let linked = peer.create::<TextContent>(inside).await.unwrap();
        peer.save(linked, &TextBlock::of("Private plan"), None)
            .await
            .unwrap();
        let shared = peer.create::<TextContent>(inside).await.unwrap();
        let url = block_url("https://example.com/", personal, linked);
        peer.save(shared, &TextBlock::of(&format!("Notes, see {url}")), None)
            .await
            .unwrap();
        peer.share(shared, harness.account, be_graph::Access::View, &public)
            .await
            .unwrap();
        (shared, linked)
    });

    ask_for(vec![
        Query::Shared,
        Query::Detached,
        Query::References(shared),
    ]);
    wait_until("heard of the linked block and what links to it", |held| {
        held.graph
            .get(linked)
            .is_some_and(|node| node.access == be_graph::Access::KnowExists)
            && held
                .graph
                .get(shared)
                .is_some_and(|node| node.references.contains(&linked))
    });
    let node = node(linked).expect("the linked block is in the mirror");
    assert_eq!(node.content_type, TextContent::CONTENT_TYPE);
    assert_eq!((node.parent, node.author, node.head), (None, None, None));
    assert!(node.metadata.name().is_none());
    assert!(
        !node.locked,
        "a block it may not view is not waiting for a key"
    );

    let listed = |found: Query| query(found).iter().any(|node| node.id == linked);
    assert!(!listed(Query::Roots), "a missing parent is not the root");
    assert!(!listed(Query::Detached), "nor Recently Deleted");
    assert!(!listed(Query::Shared), "nor something shared with it");
    assert!(
        listed(Query::References(shared)),
        "it shows where it is linked"
    );
    assert!(query(Query::Shared).iter().any(|node| node.id == shared));

    let info =
        block_editor_plugin::BlockInfo::decode(crate::plugin_graph::info_of(&node, false));
    assert!(info.no_access() && !info.locked);
    assert_eq!(info.reach(), block_editor_plugin::block_ui::Reach::NoAccess);
}
