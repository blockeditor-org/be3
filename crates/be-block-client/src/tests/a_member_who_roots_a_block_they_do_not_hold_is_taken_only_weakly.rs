use super::*;

use be_block_keys::{CarriedKey, Entry, Party, RecoveryPublic};

/// A member who cannot read a block signs a root of it naming themselves, and the server serves
/// that as its log and drops the block's genuine escrow wraps. An administrator who reaches the
/// block only through escrow cannot tell (nothing else pins its root), so it takes the member's
/// key, but weakly: its root shows as unverified (`RootPin::Weak`), the key vouches for nothing
/// and is handed on to no one, so the made-up log spreads to no other device
/// (guides/blocks/keys.md, "Accepted limits").
#[tokio::test]
async fn a_member_who_roots_a_block_they_do_not_hold_is_taken_only_weakly() {
    let harness = Harness::start().await;
    let admin = harness.owner("admin@example.com").await;
    let member = harness.member(&admin, "member@example.com").await;
    let intruder = harness.member(&admin, "intruder@example.com").await;
    let organization = admin.organization();
    let secret = workspace(&member, "Secret").await;
    let escrow = admin.keys().escrow().unwrap();

    let planted = Entry::root(
        intruder.keys().identity(),
        intruder.account().into_bytes(),
        be_block_keys::LogKind::Block,
        secret.into_bytes(),
    );
    let key = Uuid::new_v4();
    let carried = CarriedKey {
        key: [9; 32],
        block: secret.into_bytes(),
        root: Some(planted.hash()),
        heads: be_block_keys::Heads::default(),
    };
    let to = Party {
        account: escrow.id.into_bytes(),
        public: RecoveryPublic(escrow.public),
    };
    let database = harness.database();
    for statement in [
        "DELETE FROM wrapped_keys WHERE organization_id = ?1 AND wrapped_by = ?2",
        "DELETE FROM access_log_names WHERE organization_id = ?1 AND block_id = ?3",
    ] {
        let mut statement = database.prepare(statement).unwrap();
        let wanted = statement.parameter_count();
        let values = [
            organization.to_string(),
            escrow.id.to_string(),
            secret.to_string(),
        ];
        statement
            .execute(rusqlite::params_from_iter(values.iter().take(wanted)))
            .unwrap();
    }
    database
        .execute(
            "DELETE FROM access_log WHERE organization_id = ?1 AND block_id = ?2",
            rusqlite::params![organization.to_string(), secret.to_string()],
        )
        .unwrap();
    database
        .execute(
            "INSERT INTO access_log (organization_id, block_id, position, entry)
             VALUES (?1, ?2, 0, ?3)",
            rusqlite::params![
                organization.to_string(),
                secret.to_string(),
                planted.encode()
            ],
        )
        .unwrap();
    database
        .execute(
            "INSERT INTO keys (organization_id, id, block_id, kind, epoch, created)
             VALUES (?1, ?2, ?3, 'scope', 9, 0)",
            rusqlite::params![
                organization.to_string(),
                key.to_string(),
                secret.to_string()
            ],
        )
        .unwrap();
    database
        .execute(
            "INSERT INTO wrapped_keys (organization_id, key_id, wrapped_by, sender, sealed)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![
                organization.to_string(),
                key.to_string(),
                escrow.id.to_string(),
                intruder.account().to_string(),
                intruder.keys().identity().seal_for(
                    intruder.account().into_bytes(),
                    &to,
                    key.as_bytes(),
                    &carried.encode()
                )
            ],
        )
        .unwrap();
    drop(database);

    let laptop = harness.second(&admin).await;
    assert_eq!(
        laptop.keys().root_pin(secret),
        crate::keys::RootPin::Weak(planted.hash()),
        "a root only a member's wrap carries is shown as unverified"
    );
    assert!(laptop.keys().ring().contains(key));
    assert!(
        !laptop.keys().trusted(key),
        "a key taken on a member's word alone vouches for what it opens"
    );
    assert!(
        laptop.keys().carried_plain(key).is_err(),
        "a key taken on a member's word alone is handed on"
    );
    assert_eq!(
        laptop.keys().root(secret),
        None,
        "a weak root is passed on as pinned"
    );
    assert_eq!(
        member.keys().root_pin(secret),
        crate::keys::RootPin::Pinned(member.keys().root(secret).unwrap()),
        "the block's own members keep its real root"
    );

    harness.stop().await;
}
