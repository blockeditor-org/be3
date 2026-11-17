use super::*;
use be_block_graph::BlockParent;
use be_type_text::TextContent;

/// Makes a block of `C` under `parent` on `device`, sent to the server.
fn make<C: be_block_content::BlockContent>(device: &Device, parent: BlockParent) -> Uuid {
    let peer = std::sync::Arc::clone(&device.peer.borrow());
    device.runtime.block_on(async {
        let block = peer
            .create::<C>(parent)
            .await
            .expect("the other device makes a block");
        peer.drain(|_| false)
            .await
            .expect("the other device sends it");
        block
    })
}

#[test]
fn a_folder_another_device_fills_is_followed_with_only_what_is_shown_mirrored() {
    let harness = Harness::start();
    let other = Device::online(&harness);
    let shown = make::<be_type_workspace::WorkspaceContent>(&other, BlockParent::Root);
    let unshown = make::<be_type_workspace::WorkspaceContent>(&other, BlockParent::Root);

    harness.connect();
    ask_for(vec![Query::Children(shown)]);
    wait_until("listed the shown folder", |shared| {
        shared.graph.ready(Query::Children(shown))
    });
    wait_until("followed the shown folder's tree", |shared| {
        shared.graph.followed_entries().contains(&shown)
    });

    // The other device fills both folders: the tree followed advances, and what it read of it
    // that the folder lists joins the mirror.
    let elsewhere = make::<TextContent>(&other, BlockParent::Block(unshown));
    let first = make::<TextContent>(&other, BlockParent::Block(shown));
    let second = make::<TextContent>(&other, BlockParent::Block(shown));
    wait_until("mirrored what the other device made in the shown folder", |shared| {
        shared.graph.get(first).is_some() && shared.graph.get(second).is_some()
    });
    // What the other folder holds is read only if its tree is followed (the top level shown
    // here, say), and kept by no listing either way.
    let mirrored = with_shared_mut(|shared| shared.graph.get(elsewhere).is_some());
    assert_eq!(mirrored, Some(false), "only what is shown is mirrored");
}
