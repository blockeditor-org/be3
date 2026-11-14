use super::*;

use be_block_graph::{Access, SETTINGS_TYPE};
use be_block_protocol::{KeyKind, RecoveryKey, client};
use be_type_text::{TextBlock, TextContent};

/// Key wraps of `block`'s scope keys by anything but an account: a scope above, or the escrow
/// key.
fn wrapped_by_keys(harness: &Harness, block: Uuid) -> i64 {
    harness
        .database()
        .query_row(
            "SELECT COUNT(*) FROM wrapped_keys AS wrap
             JOIN keys AS key ON key.organization_id = wrap.organization_id AND key.id = wrap.key_id
             WHERE key.block_id = ?1 AND key.kind = 'scope'
                AND wrap.wrapped_by NOT LIKE 'account:%'",
            [block.to_string()],
            |row| row.get(0),
        )
        .unwrap()
}

fn rotate_later(harness: &Harness, peer: &Peer<MemoryStore>, block: Uuid) {
    harness
        .database()
        .execute(
            "INSERT OR IGNORE INTO rotations (organization_id, block_id) VALUES (?1, ?2)",
            rusqlite::params![peer.organization().to_string(), block.to_string()],
        )
        .unwrap();
}

async fn escrow_work_for(peer: &Peer<MemoryStore>, block: Uuid) -> Vec<Uuid> {
    let scope_keys: Vec<Uuid> = peer
        .keys()
        .keys_of(block, KeyKind::Scope)
        .into_iter()
        .map(|row| row.id)
        .collect();
    peer.key_work()
        .await
        .unwrap()
        .unescrowed
        .into_iter()
        .filter(|key| scope_keys.contains(key))
        .collect()
}

#[tokio::test]
async fn an_owner_only_block_is_read_by_its_author_alone() {
    let harness = Harness::start().await;
    let admin = harness.owner("admin@example.com").await;
    let member = harness.member(&admin, "member@example.com").await;
    let settings = member
        .create_block(
            Uuid::now_v7(),
            SETTINGS_TYPE,
            BlockParent::Root,
            &be_block_content::BlockMetadata::default(),
            false,
        )
        .await
        .unwrap()
        .id;
    let secret = member
        .create_block(
            Uuid::now_v7(),
            TextContent::CONTENT_TYPE,
            BlockParent::Block(settings),
            &be_block_content::BlockMetadata::default(),
            true,
        )
        .await
        .unwrap();
    assert!(secret.owner_only);
    assert_eq!(
        secret.scope,
        Some(secret.id),
        "an owner-only block is its own scope"
    );
    let secret = secret.id;
    member
        .save(secret, &TextBlock::of("a key for elsewhere"), None)
        .await
        .unwrap();
    member.tend().await.unwrap();
    admin.tend().await.unwrap();

    assert_eq!(wrapped_by_keys(&harness, secret), 0);
    assert!(escrow_work_for(&member, secret).await.is_empty());
    assert_eq!(
        admin.summary(secret).await.unwrap().access,
        Access::KnowExists,
        "an administrator knows the block exists and no more"
    );
    let listed = admin.children(BlockParent::Block(settings)).await.unwrap();
    assert!(
        listed
            .iter()
            .any(|child| child.id == secret && !child.access.can_view())
    );
    assert!(admin.open::<TextContent>(secret).await.is_err());
    assert!(
        harness
            .second(&admin)
            .await
            .open::<TextContent>(secret)
            .await
            .is_err(),
        "the escrow key opens nothing owner-only"
    );
    assert!(
        admin
            .grant(secret, admin.account(), Access::View)
            .await
            .is_err()
    );
    assert!(
        admin
            .set_parent(secret, BlockParent::Detached)
            .await
            .is_err()
    );
    assert!(
        member
            .grant(secret, admin.account(), Access::View)
            .await
            .is_err(),
        "an owner-only block is shared with no one"
    );

    let elsewhere = workspace(&member, "Elsewhere").await;
    for (maker, parent) in [(&member, elsewhere), (&admin, settings)] {
        assert!(
            maker
                .create_block(
                    Uuid::now_v7(),
                    TextContent::CONTENT_TYPE,
                    BlockParent::Block(parent),
                    &be_block_content::BlockMetadata::default(),
                    true,
                )
                .await
                .is_err(),
            "an owner-only block is made only in its author's own settings"
        );
    }
    let note = member
        .create::<TextContent>(BlockParent::Block(elsewhere))
        .await
        .unwrap();
    assert!(
        member
            .set_parent(note, BlockParent::Block(secret))
            .await
            .is_err(),
        "nothing moves into an owner-only block"
    );

    let laptop = harness.second(&member).await;
    assert_eq!(text(&laptop, secret).await, "a key for elsewhere");

    rotate_later(&harness, &member, settings);
    admin.tend().await.unwrap();
    rotate_later(&harness, &member, secret);
    member.tend().await.unwrap();
    assert!(admin.key_work().await.unwrap().rotations.is_empty());
    assert_eq!(member.keys().keys_of(secret, KeyKind::Scope).len(), 2);
    admin.tend().await.unwrap();
    assert_eq!(wrapped_by_keys(&harness, secret), 0);
    assert!(escrow_work_for(&member, secret).await.is_empty());
    assert!(
        harness
            .second(&admin)
            .await
            .open::<TextContent>(secret)
            .await
            .is_err()
    );
    assert_eq!(
        text(&harness.second(&member).await, secret).await,
        "a key for elsewhere"
    );

    let fresh = be_block_keys::RecoveryPhrase::generate().secret();
    let RecoveryKey { sealed, own, .. } =
        crate::ask(client::keys::get_recovery_key(&member.connection()))
            .await
            .unwrap();
    let resealed = crate::contacts::new_identity(
        member.account(),
        member.keys().identity(),
        &fresh,
        sealed,
        own,
    )
    .unwrap();
    crate::ask(client::keys::set_recovery_key(
        &member.connection(),
        fresh.public().0,
        fresh.signing().0,
        resealed.sealed,
        resealed.own,
        resealed.changes,
    ))
    .await
    .unwrap();
    let renewed = Peer::connect(
        PeerConfig::new(
            &harness.url,
            fresh,
            Credentials::Token(member.token().to_owned()),
        )
        .organization(Some(member.organization())),
        MemoryStore::new(),
    )
    .await
    .unwrap();
    assert_eq!(
        text(&renewed, secret).await,
        "a key for elsewhere",
        "a new recovery phrase reseals what only its owner reads"
    );

    harness.stop().await;
}
