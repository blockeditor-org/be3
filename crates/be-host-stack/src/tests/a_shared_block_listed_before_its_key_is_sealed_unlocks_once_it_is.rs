use super::*;
use be_type_workspace::WorkspaceContent;

use be_type_text::TextContent;

/// A grant reaches this account before the key it needs (as `Peer::share` sends them, or a
/// pending grant sealed later): the block shows locked, then opens as the key is sealed to it.
#[test]
fn a_shared_block_listed_before_its_key_is_sealed_unlocks_once_it_is() {
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
        .block_on(be_block_client::contacts::accept_link(
            &peer.connection(),
            &sharer.phrase.secret(),
            &sharer.me().card(),
            &code,
            be_block_client::contacts::Expected::Contact,
        ))
        .expect("the sharer accepts the link");
    runtime
        .block_on(peer.refresh_contacts())
        .expect("the sharer lists its contacts");
    let public = peer
        .keys()
        .contact(harness.account)
        .expect("the sharer verified this account through the link");

    let roadmap = runtime.block_on(async {
        let space = peer
            .create_block(
                Uuid::now_v7(),
                WorkspaceContent::CONTENT_TYPE,
                be_block_graph::BlockParent::Root,
                &be_block_content::BlockMetadata::named("Sharer's"),
                false,
            )
            .await
            .unwrap()
            .id;
        let roadmap = peer
            .create_block(
                Uuid::now_v7(),
                TextContent::CONTENT_TYPE,
                be_block_graph::BlockParent::Block(space),
                &be_block_content::BlockMetadata::named("Roadmap"),
                false,
            )
            .await
            .unwrap()
            .id;
        peer.grant(roadmap, harness.account, be_block_graph::Access::View)
            .await
            .unwrap();
        roadmap
    });

    ask_for(vec![Query::Shared]);
    wait_until("lists the shared block, locked", |held| {
        held.graph
            .get(roadmap)
            .is_some_and(|node| node.locked && node.metadata.name().is_none())
    });

    runtime
        .block_on(peer.seal_share(roadmap, harness.account, &public))
        .expect("the sharer seals the block's key to this account");
    wait_until("opens the block once its key is sealed", |held| {
        held.graph
            .get(roadmap)
            .is_some_and(|node| !node.locked && node.metadata.name().as_deref() == Some("Roadmap"))
    });
}
