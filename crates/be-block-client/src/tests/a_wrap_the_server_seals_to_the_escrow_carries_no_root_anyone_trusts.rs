use super::*;

use be_block_keys::{CarriedKey, Entry, Party, RecoveryPublic};

/// Puts a scope key of `block` with an escrow wrap sealed as `sealed` from `sender` straight into
/// the server's database, as a server that makes one up would. Answers the key.
fn plant(
    harness: &Harness,
    organization: Uuid,
    block: Uuid,
    epoch: i64,
    escrow: Uuid,
    sender: Uuid,
    sealed: Vec<u8>,
    key: Uuid,
) -> Uuid {
    let database = harness.database();
    database
        .execute(
            "INSERT INTO keys (organization_id, id, block_id, kind, epoch, created)
             VALUES (?1, ?2, ?3, 'scope', ?4, 0)",
            rusqlite::params![organization.to_string(), key.to_string(), block.to_string(), epoch],
        )
        .unwrap();
    database
        .execute(
            "INSERT INTO wrapped_keys (organization_id, key_id, wrapped_by, sender, sealed)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![
                organization.to_string(),
                key.to_string(),
                escrow.to_string(),
                sender.to_string(),
                sealed
            ],
        )
        .unwrap();
    key
}

/// An escrow wrap is sealed from its sender, and an administrator opens it only from a member
/// the organization's log names, who the block's log names with Edit or above; the root it
/// carries is pinned only then (signed access 3a). Neither the server, sealing anonymously to
/// the escrow key, nor a member who cannot edit the block makes a key, or a root, an
/// administrator takes; the member's genuine wrap pins the real root, so an administrator who
/// reaches the block only through escrow checks its log, and its members keep their pin.
#[tokio::test]
async fn a_wrap_the_server_seals_to_the_escrow_carries_no_root_anyone_trusts() {
    let harness = Harness::start().await;
    let admin = harness.owner("admin@example.com").await;
    let member = harness.member(&admin, "member@example.com").await;
    let stranger = harness.member(&admin, "stranger@example.com").await;
    let organization = admin.organization();
    let team = workspace(&member, "Team").await;
    let real = member.keys().root(team).unwrap();
    let escrow = admin.keys().escrow().unwrap();
    let forged = Entry::root(
        stranger.keys().identity(),
        stranger.account().into_bytes(),
        be_block_keys::LogKind::Block,
        team.into_bytes(),
    );
    let carried = CarriedKey {
        key: [9; 32],
        block: team.into_bytes(),
        root: Some(forged.hash()),
    };
    let carrying_real = CarriedKey {
        root: Some(real),
        ..carried
    };

    // The server seals a key of the workspace carrying a root of its making, as from the member.
    let anonymous = plant(
        &harness,
        organization,
        team,
        9,
        escrow.id,
        member.account(),
        RecoveryPublic(escrow.public).seal_anonymous(&carried.encode()),
        Uuid::new_v4(),
    );
    // A member who cannot edit the workspace seals one, truly from themselves, carrying its real
    // root.
    let key = Uuid::new_v4();
    let to = Party {
        account: escrow.id.into_bytes(),
        public: RecoveryPublic(escrow.public),
    };
    let unentitled = plant(
        &harness,
        organization,
        team,
        10,
        escrow.id,
        stranger.account(),
        stranger.keys().identity().seal_for(
            stranger.account().into_bytes(),
            &to,
            key.as_bytes(),
            &carrying_real.encode(),
        ),
        key,
    );

    // A member the server adds, whom no administrator signed in, seals one from themselves.
    let added = harness.owner("added@example.com").await;
    harness
        .database()
        .execute(
            "INSERT INTO memberships (organization_id, account_id, role) VALUES (?1, ?2, 'editor')",
            rusqlite::params![organization.to_string(), added.account().to_string()],
        )
        .unwrap();
    let key = Uuid::new_v4();
    let unsigned = plant(
        &harness,
        organization,
        team,
        11,
        escrow.id,
        added.account(),
        added.keys().identity().seal_for(
            added.account().into_bytes(),
            &to,
            key.as_bytes(),
            &carried.encode(),
        ),
        key,
    );

    let laptop = harness.second(&admin).await;
    for planted in [anonymous, unentitled, unsigned] {
        assert!(
            !laptop.keys().ring().contains(planted),
            "an administrator took a key from a wrap no member entitled to it sealed"
        );
    }
    assert_eq!(
        laptop.keys().root_pin(team),
        crate::keys::RootPin::Pinned(real),
        "the member's genuine wrap pins the real root, and no other root was taken"
    );
    assert!(
        laptop.access_log(team).await.is_ok(),
        "the administrator checks the log of a block it reaches only through escrow"
    );
    let reread = harness.second(&member).await;
    assert_eq!(
        reread.keys().root_pin(team),
        crate::keys::RootPin::Pinned(real),
        "the member's root moved, or conflicted"
    );

    // A member seals one carrying another root than the one the block's genuine wraps carry: the
    // administrator's device shows the block's root as conflicting, rather than taking either
    // quietly by which came first.
    let key = Uuid::new_v4();
    plant(
        &harness,
        organization,
        team,
        12,
        escrow.id,
        stranger.account(),
        stranger.keys().identity().seal_for(
            stranger.account().into_bytes(),
            &to,
            key.as_bytes(),
            &carried.encode(),
        ),
        key,
    );
    let again = harness.second(&admin).await;
    assert_eq!(
        again.keys().root_pin(team),
        crate::keys::RootPin::Conflicting,
        "a root that disagrees with the genuine one was skipped silently"
    );
    assert!(!again.keys().ring().contains(key));

    harness.stop().await;
}
