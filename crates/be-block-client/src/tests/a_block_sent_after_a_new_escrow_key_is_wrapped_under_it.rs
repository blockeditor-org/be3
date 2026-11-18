use super::*;

use be_block_protocol::WrappedBy;

/// A block made under one escrow key and taken by the server after the next was adopted is
/// wrapped under the next too, so the escrow it is left openable by is never only the old one
/// (whose holders a rotation took off).
#[tokio::test]
async fn a_block_sent_after_a_new_escrow_key_is_wrapped_under_it() {
    let harness = Harness::start().await;
    let admin = harness.owner("admin@example.com").await;
    let made = Uuid::now_v7();
    admin
        .keep_creation(
            made,
            be_type_workspace::WorkspaceContent::CONTENT_TYPE,
            BlockParent::Root,
            &be_block_content::BlockMetadata::named("Made before"),
            false,
        )
        .await
        .unwrap();

    let (minted, keys) = admin
        .keys()
        .mint_escrow(admin.checked_organization().await.unwrap().unwrap().head)
        .unwrap();
    be_block_protocol::write::<(), _>(
        &admin.connection(),
        Some(admin.organization()),
        be_block_protocol::Op::PutKeys(be_block_protocol::PutKeys::Escrow {
            escrow: minted.clone(),
            keys: keys,
        }),
    )
    .await
    .unwrap()
    .unwrap();
    admin.keys().adopt_escrow(minted.clone());
    admin.drain(|_| false).await.unwrap();

    let scope = admin.keys().scope_key(made).unwrap();
    let wrapped = admin.keys().wrapped_by(scope);
    assert!(wrapped.contains(&WrappedBy::Key(minted.id)), "{wrapped:?}");
    let work = admin.key_work().await.unwrap();
    assert!(!work.unescrowed.contains(&scope), "{:?}", work.unescrowed);

    harness.stop().await;
}
