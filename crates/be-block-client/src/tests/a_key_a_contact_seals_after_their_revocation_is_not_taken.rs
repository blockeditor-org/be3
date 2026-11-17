use super::*;

use be_block_graph::Access;

/// Seals `carried`, a key of `block` with id `key`, from `sender` to `to`, straight into the
/// server's database, as the server would hold a seal the sender made.
fn plant(
    harness: &Harness,
    sender: &Peer<MemoryStore>,
    to: &Peer<MemoryStore>,
    key: Uuid,
    block: Uuid,
    carried: &be_block_keys::CarriedKey,
) {
    let organization = sender.organization();
    let sealed = sender.keys().identity().seal_for(
        sender.account().into_bytes(),
        &be_block_keys::Party {
            account: to.account().into_bytes(),
            public: to.keys().identity().public(),
        },
        key.as_bytes(),
        &carried.encode(),
    );
    let database = harness.database();
    database
        .execute(
            "INSERT INTO keys (organization_id, id, block_id, kind, epoch, created)
             VALUES (?1, ?2, ?3, 'scope', 9, 0)",
            rusqlite::params![organization.to_string(), key.to_string(), block.to_string()],
        )
        .unwrap();
    database
        .execute(
            "INSERT INTO wrapped_keys (organization_id, key_id, wrapped_by, sender, sealed)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![
                organization.to_string(),
                key.to_string(),
                format!("account:{}", to.account()),
                sender.account().to_string(),
                sealed
            ],
        )
        .unwrap();
}

/// A device takes a key a contact seals it only if the block's log grants that contact Edit or
/// above as of the head the key's epoch names, or as of the log's head here when the key names
/// none this device's log reaches (signed access 3d). A contact revoked from the block makes up
/// keys and seals them to an editor: naming a head after their revocation, no head, or a head
/// the log has no entry for, none is taken; nor is one from a contact who only ever viewed. One
/// naming a head from before, while they still edited, is taken: which epoch a device writes
/// under is checked against the log (`KeyBook::outdated`), not who sealed it.
#[tokio::test]
async fn a_key_a_contact_seals_after_their_revocation_is_not_taken() {
    let harness = Harness::start().await;
    let admin = harness.owner("admin@example.com").await;
    let carol = harness.member(&admin, "carol@example.com").await;
    let bob = harness.member(&admin, "bob@example.com").await;
    let vera = harness.member(&admin, "vera@example.com").await;
    befriend(&carol, &bob).await;
    befriend(&carol, &vera).await;
    let team = workspace(&admin, "Team").await;
    for (member, access) in [
        (&carol, Access::Edit),
        (&bob, Access::Edit),
        (&vera, Access::View),
    ] {
        admin.grant(team, member.account(), access).await.unwrap();
    }
    admin.tend().await.unwrap();
    carol.refresh_keys(None).await.unwrap();
    let (entries, _) = carol.checked_access(team).await.unwrap();
    let editing = entries.last().unwrap().hash();
    admin
        .grant(team, bob.account(), Access::None)
        .await
        .unwrap();
    let (entries, _) = carol.checked_access(team).await.unwrap();
    let revoked = entries.last().unwrap().hash();

    let made_up = |head| be_block_keys::CarriedKey {
        key: [7; 32],
        block: team.into_bytes(),
        root: carol.keys().root(team),
        heads: be_block_keys::Heads {
            block: head,
            organization: None,
        },
    };
    let mut refused = Vec::new();
    for (sender, head) in [
        (&bob, Some(revoked)),
        (&bob, None),
        (&bob, Some([9; 32])),
        (&vera, Some(editing)),
    ] {
        let key = Uuid::new_v4();
        plant(&harness, sender, &carol, key, team, &made_up(head));
        refused.push(key);
    }
    let before = Uuid::new_v4();
    plant(&harness, &bob, &carol, before, team, &made_up(Some(editing)));
    carol.refresh_keys(None).await.unwrap();
    for key in refused {
        assert!(
            !carol.keys().has(key),
            "a key a contact who does not edit as of its head sealed was taken"
        );
    }
    assert!(carol.keys().trusted(before));

    harness.stop().await;
}
