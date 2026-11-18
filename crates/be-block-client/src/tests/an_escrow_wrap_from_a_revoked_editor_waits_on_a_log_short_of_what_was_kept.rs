use super::*;

/// An administrator's device checked the team's log past the revocation of Bob, an editor, then
/// restarts and is served it short of the revocation. Bob, with the server, wraps a made-up
/// key of the note to the escrow key naming the head the device kept: the device takes nothing
/// from the short log, so it does not find Bob granted and the key waits, never taken or
/// written under.
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
        label: crate::tests::planted(made_up, "data", 90),
        block: revoked.note.into_bytes(),
        root: None,
        heads: Heads {
            block: Some(kept),
            organization: None,
        },
    };
    let sealed = revoked
        .bob
        .keys()
        .identity()
        .seal_for(
            revoked.bob.account().into_bytes(),
            &Party {
                account: escrow.id.into_bytes(),
                public: RecoveryPublic(escrow.public),
            },
            made_up.as_bytes(),
            &carried.encode(),
        )
        .unwrap();
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
