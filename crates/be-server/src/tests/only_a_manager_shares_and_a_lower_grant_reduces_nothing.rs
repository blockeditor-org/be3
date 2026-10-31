use super::*;
use be_graph::Access;

#[tokio::test]
async fn only_a_manager_shares_and_a_lower_grant_reduces_nothing() {
    let harness = Harness::start().await;
    let mut owner = harness.client().await;
    owner.register("owner@example.com").await;
    owner.personal().await;
    let mut editor = harness.client().await;
    let editor_id = editor.register("editor@example.com").await;
    editor.personal().await;
    let mut third = harness.client().await;
    let third_id = third.register("third@example.com").await;
    third.personal().await;
    let link = owner.link(None).await;
    let _ = editor.accept(link, &[]).await;
    let link = owner.link(None).await;
    let _ = third.accept(link, &[]).await;
    let link = editor.link(None).await;
    let _ = third.accept(link, &[]).await;
    let workspace = owner.create_block(BlockParent::Root).await;
    let folder = owner.create_block(BlockParent::Block(workspace)).await;
    assert_eq!(owner.read_block(folder).await.access, Access::Manage);

    let granted = owner.grant(workspace, editor_id, Access::Edit).await;
    assert!(matches!(granted, Ok(())), "{granted:?}");
    assert_eq!(editor.read_block(folder).await.access, Access::Edit);
    let shared = editor.grant(folder, third_id, Access::View).await;
    assert!(
        refused_as(&shared, ErrorCode::PermissionDenied),
        "an editor shared a block: {shared:?}"
    );
    let listed = editor
        .call(|peer| client::blocks::list_access(peer, folder))
        .await;
    assert!(
        refused_as(&listed, ErrorCode::PermissionDenied),
        "an editor listed who has access: {listed:?}"
    );
    let own = editor.create_block(BlockParent::Block(folder)).await;
    assert_eq!(editor.read_block(own).await.access, Access::Manage);

    for (above, below) in [
        (Access::View, Access::KnowExists),
        (Access::Edit, Access::View),
        (Access::Manage, Access::Edit),
    ] {
        let granted = owner.grant(workspace, editor_id, above).await;
        assert!(matches!(granted, Ok(())), "{granted:?}");
        let granted = owner.grant(folder, editor_id, below).await;
        assert!(matches!(granted, Ok(())), "{granted:?}");
        assert_eq!(editor.read_block(folder).await.access, above);
    }

    let shared = editor.grant(folder, third_id, Access::View).await;
    assert!(matches!(shared, Ok(())), "{shared:?}");
    assert_eq!(third.read_block(folder).await.access, Access::View);
    let revoked = editor.grant(folder, third_id, Access::None).await;
    assert!(matches!(revoked, Ok(())), "{revoked:?}");
    let response = owner.call(|peer| client::keys::get_key_work(peer)).await;
    let Ok(work) = response else {
        panic!("reading key work failed: {response:?}");
    };
    assert_eq!(work.rotations, [folder]);

    harness.stop().await;
}
