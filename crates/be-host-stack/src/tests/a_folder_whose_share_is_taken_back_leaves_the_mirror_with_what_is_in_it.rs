use super::*;
use be_block_graph::BlockParent;

/// A folder shared with this account, shown open with what is in it: once the sharer takes the
/// share back, the folder and everything shown of it leave the mirror, though nothing here asked.
#[test]
fn a_folder_whose_share_is_taken_back_leaves_the_mirror_with_what_is_in_it() {
    let harness = Harness::relayed();
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
        .verified_identity(harness.account)
        .expect("the sharer verified this account through the link");
    let (folder, inside) = runtime.block_on(async {
        let folder = peer
            .create::<be_type_workspace::WorkspaceContent>(BlockParent::Root)
            .await
            .unwrap();
        let inside = peer
            .create::<CounterContent>(BlockParent::Block(folder))
            .await
            .unwrap();
        peer.share(
            folder,
            harness.account,
            be_block_graph::Access::View,
            &public,
        )
        .await
        .unwrap();
        (folder, inside)
    });

    ask_for(vec![Query::Shared, Query::Children(folder)]);
    wait_until("showed the shared folder and what is in it", |held| {
        held.graph.get(folder).is_some() && held.graph.get(inside).is_some()
    });
    wait_until("followed the folder's tree", |held| {
        held.graph.followed_entries().contains(&folder)
    });

    runtime
        .block_on(peer.grant(folder, harness.account, be_block_graph::Access::None))
        .expect("the sharer takes the share back");
    wait_until("let go of the folder and what is in it", |held| {
        held.graph.get(folder).is_none() && held.graph.get(inside).is_none()
    });
}
