use super::*;

use be_block_keys::{CarriedKey, Entry, RecoveryPublic};
use be_block_protocol::WrappedBy;

/// Anyone who knows an organization's escrow public key can seal to it, the server included, so
/// a root carried by a key opened only through escrow pins nothing. An administrator who reaches
/// a block only that way and rotates its keys hands them on with no root, so the server cannot
/// launder a root it planted into one an administrator seems to vouch for: the block's members
/// keep the root they pinned, with no conflict.
#[tokio::test]
async fn a_root_an_administrator_reaches_only_through_escrow_is_never_passed_on() {
    let harness = Harness::start().await;
    let admin = harness.owner("admin@example.com").await;
    let member = harness.member(&admin, "member@example.com").await;
    let stranger = harness.member(&admin, "stranger@example.com").await;
    let organization = admin.organization();
    let team = workspace(&member, "Team").await;
    let real = member.keys().root(team).unwrap();

    // The server seals a key of the workspace carrying a root of its making to the escrow.
    let escrow = admin.keys().escrow().unwrap();
    let forged = Entry::root(
        stranger.keys().identity(),
        stranger.account().into_bytes(),
        be_block_keys::LogKind::Block,
        team.into_bytes(),
    );
    let planted = Uuid::new_v4();
    let carried = CarriedKey {
        key: [9; 32],
        block: team.into_bytes(),
        root: Some(forged.hash()),
    };
    let database = harness.database();
    database
        .execute(
            "INSERT INTO keys (organization_id, id, block_id, kind, epoch, created)
             VALUES (?1, ?2, ?3, 'scope', 9, 0)",
            rusqlite::params![organization.to_string(), planted.to_string(), team.to_string()],
        )
        .unwrap();
    database
        .execute(
            "INSERT INTO wrapped_keys (organization_id, key_id, wrapped_by, sender, sealed)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![
                organization.to_string(),
                planted.to_string(),
                escrow.id.to_string(),
                member.account().to_string(),
                RecoveryPublic(escrow.public).seal_anonymous(&carried.encode())
            ],
        )
        .unwrap();
    database
        .execute(
            "INSERT OR IGNORE INTO rotations (organization_id, block_id) VALUES (?1, ?2)",
            rusqlite::params![organization.to_string(), team.to_string()],
        )
        .unwrap();
    drop(database);

    admin.refresh_keys(None).await.unwrap();
    assert_eq!(
        admin.keys().root(team),
        None,
        "a root carried only through escrow was pinned"
    );
    admin.tend().await.unwrap();
    let rotated: Vec<_> = admin
        .keys()
        .keys_of(team, be_block_protocol::KeyKind::Scope)
        .into_iter()
        .filter(|row| row.epoch > 9)
        .collect();
    assert!(
        !rotated.is_empty(),
        "the administrator rotated the workspace's keys"
    );
    assert!(
        rotated.iter().any(|row| admin
            .keys()
            .wrapped_by(row.id)
            .contains(&WrappedBy::Account(member.account()))),
        "and sealed the new scope key to the member"
    );

    let secret = be_block_keys::EscrowSecret::from_bytes(
        *admin.keys().ring().get(escrow.id).unwrap().as_bytes(),
    );
    for row in &rotated {
        let sealed: Vec<u8> = harness
            .database()
            .query_row(
                "SELECT sealed FROM wrapped_keys WHERE key_id = ?1 AND wrapped_by = ?2",
                rusqlite::params![row.id.to_string(), escrow.id.to_string()],
                |found| found.get(0),
            )
            .unwrap();
        let carried = CarriedKey::decode(&secret.open(&sealed).unwrap()).unwrap();
        assert_eq!(
            (carried.block, carried.root),
            (team.into_bytes(), None),
            "the administrator's rotated key carried a root it holds no pin of"
        );
    }

    let laptop = harness.second(&member).await;
    assert_eq!(
        laptop.keys().root_pin(team),
        crate::keys::RootPin::Pinned(real),
        "the member's root moved, or conflicted, with keys the administrator rotated"
    );
    assert!(laptop.access_log(team).await.is_ok());

    harness.stop().await;
}
