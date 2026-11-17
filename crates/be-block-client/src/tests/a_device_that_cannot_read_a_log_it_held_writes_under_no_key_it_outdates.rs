use super::*;

use be_block_graph::Access;
use be_block_keys::{CarriedKey, EntryHash, Heads};
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
    admin
        .grant(team, bob.account(), Access::View)
        .await
        .unwrap();
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
/// the log when it was last online, which holds the revocation. It cannot give the team a new
/// epoch offline, so the save fails as locked rather than write under the key Bob made up
/// (signed access 3d).
#[tokio::test]
async fn an_offline_device_fails_a_save_rather_than_write_under_a_made_up_key() {
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
        matches!(saved, Err(ClientError::Locked(_))),
        "an offline save was not refused: {saved:?}"
    );
    assert_eq!(offline.pending().operations, 0);

    harness.stop().await;
}
