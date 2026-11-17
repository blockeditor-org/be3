use super::*;

use be_block_graph::Access;
use be_block_keys::{CarriedKey, EntryHash, Heads, Party, RecoveryPublic};
use be_block_protocol::KeyKind;
use be_type_text::{TextBlock, TextContent};


/// A team where `bob` viewed and was revoked, and the epoch after: `(admin, carol, bob, team,
/// note, head, old)`, `old` the scope epoch Bob still holds.
struct Revoked {
    admin: Peer<MemoryStore>,
    carol: Peer<MemoryStore>,
    bob: Peer<MemoryStore>,
    team: Uuid,
    note: Uuid,
    head: Option<be_block_commit::CommitId>,
    old: Uuid,
}

async fn revoked(harness: &Harness) -> Revoked {
    revoked_from(harness, Access::View).await
}

/// `revoked`, with Bob granted `access` before his revocation.
async fn revoked_from(harness: &Harness, access: Access) -> Revoked {
    let admin = harness.owner("admin@example.com").await;
    let carol = harness.member(&admin, "carol@example.com").await;
    let bob = harness.member(&admin, "bob@example.com").await;
    let team = workspace(&admin, "Team").await;
    let note = admin
        .create::<TextContent>(BlockParent::Block(team))
        .await
        .unwrap();
    let head = admin
        .save(note, &TextBlock::of("before"), None)
        .await
        .unwrap()
        .published();
    admin
        .grant(team, carol.account(), Access::Edit)
        .await
        .unwrap();
    admin.grant(team, bob.account(), access).await.unwrap();
    admin.tend().await.unwrap();
    bob.refresh_keys(None).await.unwrap();
    let old = bob.keys().scope_key(team).unwrap();
    admin
        .grant(team, bob.account(), Access::None)
        .await
        .unwrap();
    admin.tend().await.unwrap();
    Revoked {
        admin,
        carol,
        bob,
        team,
        note,
        head,
        old,
    }
}

/// Bob, with the server, makes up a data key of the note naming `named`, wraps it under the
/// epoch he still holds and has it listed as the note's newest.
fn plant(harness: &Harness, revoked: &Revoked, named: Option<EntryHash>) -> Uuid {
    let made_up = Uuid::new_v4();
    let carried = CarriedKey {
        key: [5; 32],
        block: revoked.note.into_bytes(),
        root: None,
        heads: Heads {
            block: named,
            organization: None,
        },
    };
    let sealed = revoked
        .bob
        .keys()
        .ring()
        .seal(revoked.old, &carried.encode())
        .unwrap();
    let organization = revoked.admin.organization();
    let database = harness.database();
    database
        .execute(
            "INSERT INTO keys (organization_id, id, block_id, kind, epoch, created)
             VALUES (?1, ?2, ?3, 'data', 90, 0)",
            rusqlite::params![
                organization.to_string(),
                made_up.to_string(),
                revoked.note.to_string()
            ],
        )
        .unwrap();
    database
        .execute(
            "INSERT INTO wrapped_keys (organization_id, key_id, wrapped_by, sender, sealed)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![
                organization.to_string(),
                made_up.to_string(),
                revoked.old.to_string(),
                revoked.bob.account().to_string(),
                sealed
            ],
        )
        .unwrap();
    made_up
}

/// Saves as `device`, which opened `made_up`, and checks the save gave the team a new epoch
/// first and wrote under a key Bob never gets.
async fn saves_past(revoked: &Revoked, device: &Peer<MemoryStore>, made_up: Uuid) {
    assert!(
        device.keys().trusted(made_up),
        "the made-up key did not open, so the test shows nothing"
    );
    let epochs = device.keys().keys_of(revoked.team, KeyKind::Scope).len();
    device
        .save(revoked.note, &TextBlock::of("after"), revoked.head)
        .await
        .unwrap();
    assert_eq!(
        device.keys().keys_of(revoked.team, KeyKind::Scope).len(),
        epochs + 1,
        "the save wrote under the made-up key"
    );
    let written = device
        .keys()
        .keys_of(revoked.note, KeyKind::Data)
        .last()
        .unwrap()
        .id;
    assert_ne!(written, made_up);
    revoked.bob.refresh_keys(None).await.unwrap();
    assert!(!revoked.bob.keys().has(written));
}

/// A device checked the team's log past Bob's revocation, then restarts, and the server serves
/// it the log only up to before the revocation: it takes nothing from that log, judges by what
/// it kept of the earlier one, and gives the team a new epoch rather than write under the key
/// Bob made up (signed access 3d).
#[tokio::test]
async fn a_restarted_device_served_a_log_before_a_revocation_it_held_writes_under_no_made_up_key()
{
    let harness = Harness::start().await;
    let revoked = revoked(&harness).await;
    let refs = refs::RefsDb::memory();
    let store = MemoryStore::new();
    let before = harness.device(&revoked.carol, &refs, &store).await;
    before.checked_access(revoked.team).await.unwrap();
    drop(before);

    let made_up = plant(&harness, &revoked, None);
    let withheld = withhold_newest(&harness, revoked.admin.organization(), Some(revoked.team));
    let restarted = harness.device(&revoked.carol, &refs, &store).await;
    saves_past(&revoked, &restarted, made_up).await;
    withheld.serve(&harness);

    harness.stop().await;
}

/// A device offline, whose read of the team's log fails, judges the epoch by what it kept of
/// the log when it was last online, which holds the revocation, and cannot give the team a new
/// epoch. A person's edit made offline is never refused: it is sealed and kept here. But it is
/// judged again before it is sent, and set aside rather than sent under the key Bob made up
/// (signed access 3d).
#[tokio::test]
async fn an_edit_made_offline_under_a_made_up_key_is_kept_and_never_sent() {
    let harness = Harness::start().await;
    let revoked = revoked(&harness).await;
    let made_up = plant(&harness, &revoked, None);
    let refs = refs::RefsDb::memory();
    let store = MemoryStore::new();
    let online = harness.device(&revoked.carol, &refs, &store).await;
    assert_eq!(text(&online, revoked.note).await, "before");
    assert!(online.keys().trusted(made_up));
    drop(online);

    let offline = harness.offline(&revoked.carol, &refs, &store);
    assert!(
        offline.keys().trusted(made_up),
        "the made-up key did not open, so the test shows nothing"
    );
    let saved = offline
        .save(revoked.note, &TextBlock::of("offline"), revoked.head)
        .await;
    assert!(
        matches!(saved, Ok(Saved::Kept(_))),
        "an edit made offline was refused: {saved:?}"
    );
    assert_eq!(offline.pending().operations, 1);
    drop(offline);

    let back = harness.device(&revoked.carol, &refs, &store).await;
    let drained = back.drain(|_| false).await.unwrap();
    assert!(
        drained
            .iter()
            .any(|outcome| matches!(outcome, Drained::SetAside { block, .. } if *block == revoked.note)),
        "the edit sealed under the made-up key was not set aside: {drained:?}"
    );
    assert_eq!(
        revoked.admin.summary(revoked.note).await.unwrap().head,
        revoked.head,
        "the edit sealed under the made-up key was sent"
    );

    harness.stop().await;
}

/// Puts a key of `block` with id `key` and `kind`, sealed by Bob as `sealed`, in the server's
/// database, wrapped by `wrapped_by` (an account or a key, as the server writes it).
fn plant_sealed(
    harness: &Harness,
    revoked: &Revoked,
    key: Uuid,
    block: Uuid,
    kind: &str,
    wrapped_by: String,
    sealed: Vec<u8>,
) {
    let organization = revoked.admin.organization();
    let database = harness.database();
    database
        .execute(
            "INSERT INTO keys (organization_id, id, block_id, kind, epoch, created)
             VALUES (?1, ?2, ?3, ?4, 90, 0)",
            rusqlite::params![
                organization.to_string(),
                key.to_string(),
                block.to_string(),
                kind
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
                wrapped_by,
                revoked.bob.account().to_string(),
                sealed
            ],
        )
        .unwrap();
}

/// The head of the team's log, past Bob's revocation, as `device` checks it.
async fn team_head(revoked: &Revoked, device: &Peer<MemoryStore>) -> EntryHash {
    device.checked_access(revoked.team).await.unwrap().1.head
}

/// An administrator's device checked the team's log past the revocation of Bob, an editor, then
/// restarts and is served it short of the revocation. Bob, with the server, wraps a made-up
/// key of the note to the escrow key naming the head the device kept: the device takes nothing
/// from the short log, so it does not find Bob granted and the key waits, never taken or
/// written under (signed access 3d).
#[tokio::test]
async fn an_escrow_wrap_from_a_revoked_editor_waits_on_a_log_short_of_what_was_kept() {
    let harness = Harness::start().await;
    let revoked = revoked_from(&harness, Access::Edit).await;
    let refs = refs::RefsDb::memory();
    let store = MemoryStore::new();
    let before = harness.device(&revoked.admin, &refs, &store).await;
    let kept = team_head(&revoked, &before).await;
    drop(before);

    let escrow = revoked.admin.keys().escrow().unwrap();
    let made_up = Uuid::new_v4();
    let carried = CarriedKey {
        key: [6; 32],
        block: revoked.note.into_bytes(),
        root: None,
        heads: Heads {
            block: Some(kept),
            organization: None,
        },
    };
    let sealed = revoked.bob.keys().identity().seal_for(
        revoked.bob.account().into_bytes(),
        &Party {
            account: escrow.id.into_bytes(),
            public: RecoveryPublic(escrow.public),
        },
        made_up.as_bytes(),
        &carried.encode(),
    );
    plant_sealed(
        &harness,
        &revoked,
        made_up,
        revoked.note,
        "data",
        escrow.id.to_string(),
        sealed,
    );
    let withheld = withhold_newest(&harness, revoked.admin.organization(), Some(revoked.team));
    let restarted = harness.device(&revoked.admin, &refs, &store).await;
    assert!(
        restarted.checked_access(revoked.team).await.is_err(),
        "a log short of what was kept was taken"
    );
    assert!(
        !restarted.keys().has(made_up),
        "an escrow wrap from a revoked editor was taken on a short log"
    );
    restarted
        .save(revoked.note, &TextBlock::of("after"), revoked.head)
        .await
        .unwrap();
    assert!(
        !restarted.keys().has(made_up),
        "the save took the escrow wrap from a revoked editor"
    );
    withheld.serve(&harness);

    harness.stop().await;
}

/// An editor's device checked the team's log past the revocation of Bob, a contact, then
/// restarts and is served it short of the revocation. Bob seals it made-up keys naming the head
/// it kept, a scope key of the team and a data key of the note: neither is taken while its log
/// stays short, the data key never from a contact who does not administer (signed access 3d).
#[tokio::test]
async fn a_contacts_seal_waits_on_a_log_short_of_what_was_kept() {
    let harness = Harness::start().await;
    let revoked = revoked(&harness).await;
    befriend(&revoked.carol, &revoked.bob).await;
    let refs = refs::RefsDb::memory();
    let store = MemoryStore::new();
    let before = harness.device(&revoked.carol, &refs, &store).await;
    let kept = team_head(&revoked, &before).await;
    drop(before);

    let mut planted = Vec::new();
    for (block, kind) in [(revoked.team, "scope"), (revoked.note, "data")] {
        let key = Uuid::new_v4();
        let carried = CarriedKey {
            key: [8; 32],
            block: block.into_bytes(),
            root: None,
            heads: Heads {
                block: Some(kept),
                organization: None,
            },
        };
        let sealed = revoked.bob.keys().identity().seal_for(
            revoked.bob.account().into_bytes(),
            &Party {
                account: revoked.carol.account().into_bytes(),
                public: revoked.carol.keys().identity().public(),
            },
            key.as_bytes(),
            &carried.encode(),
        );
        plant_sealed(
            &harness,
            &revoked,
            key,
            block,
            kind,
            format!("account:{}", revoked.carol.account()),
            sealed,
        );
        planted.push(key);
    }
    let withheld = withhold_newest(&harness, revoked.admin.organization(), Some(revoked.team));
    let restarted = harness.device(&revoked.carol, &refs, &store).await;
    restarted.refresh_keys(None).await.unwrap();
    for key in planted {
        assert!(
            !restarted.keys().has(key),
            "a revoked contact's seal was taken on a short log"
        );
    }
    withheld.serve(&harness);

    harness.stop().await;
}

/// A device kept every entry of the team's log from its last lowering through its head:
/// offline, a key naming one between them, made after the revocation and before a later grant,
/// is current, and the save goes through (signed access 3d).
#[tokio::test]
async fn an_offline_save_under_a_key_named_between_the_last_lowering_and_the_head_goes_through()
{
    let harness = Harness::start().await;
    let admin = harness.owner("admin@example.com").await;
    let carol = harness.member(&admin, "carol@example.com").await;
    let bob = harness.member(&admin, "bob@example.com").await;
    let dave = harness.member(&admin, "dave@example.com").await;
    let erin = harness.member(&admin, "erin@example.com").await;
    let team = workspace(&admin, "Team").await;
    let note = admin
        .create::<TextContent>(BlockParent::Block(team))
        .await
        .unwrap();
    let head = admin
        .save(note, &TextBlock::of("before"), None)
        .await
        .unwrap()
        .published();
    for (member, access) in [(&carol, Access::Edit), (&bob, Access::View)] {
        admin.grant(team, member.account(), access).await.unwrap();
    }
    admin.tend().await.unwrap();
    // The revocation, a grant, the rotation (naming the grant's head), then another grant.
    admin
        .grant(team, bob.account(), Access::None)
        .await
        .unwrap();
    admin
        .grant(team, dave.account(), Access::View)
        .await
        .unwrap();
    admin.tend().await.unwrap();
    let (entries, _) = admin.checked_access(team).await.unwrap();
    let between = entries.last().unwrap().hash();
    admin
        .grant(team, erin.account(), Access::View)
        .await
        .unwrap();

    let refs = refs::RefsDb::memory();
    let store = MemoryStore::new();
    let online = harness.device(&carol, &refs, &store).await;
    assert_eq!(text(&online, note).await, "before");
    let current = online.keys().keys_of(note, KeyKind::Data).last().unwrap().id;
    assert_eq!(online.keys().heads(current).block, Some(between));
    drop(online);

    let offline = harness.offline(&carol, &refs, &store);
    let saved = offline.save(note, &TextBlock::of("offline"), head).await;
    assert!(saved.is_ok(), "an offline save under a current key failed: {saved:?}");

    harness.stop().await;
}
