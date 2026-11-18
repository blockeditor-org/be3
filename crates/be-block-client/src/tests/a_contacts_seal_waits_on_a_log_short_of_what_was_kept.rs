use super::*;

/// An editor's device checked the team's log past the revocation of Bob, a contact, then
/// restarts and is served it short of the revocation. Bob seals it made-up keys naming the head
/// it kept, a scope key of the team and a data key of the note: neither is taken while its log
/// stays short, the data key never from a contact who does not administer.
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
            label: crate::tests::planted(key, kind, 90),
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
        ).unwrap();
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
