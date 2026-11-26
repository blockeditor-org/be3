use super::*;

/// An owner's device revokes a reader; the server takes it, and the write is dropped before its
/// answer arrives, the device restarting from what it kept as after the app quit. The team's
/// log was marked unsure before the revocation was sent, so the restarted device judges no edit
/// by the log it kept from before: an offline save is kept for the outbox, and the drain once
/// connected, while the server withholds the log, sends nothing under the epoch the reader
/// holds; once a read answers, the edit is sealed again under a new epoch.
#[tokio::test]
async fn an_owner_s_offline_save_after_its_revocation_was_dropped_unanswered_stays_unsent_across_a_restart()
 {
    let lost = LostRevocation::losing(Lose::Dropped).await;
    let epochs = lost.tablet.keys().keys_of(lost.team, KeyKind::Scope).len();
    assert!(matches!(
        lost.tablet
            .save(lost.note, &TextBlock::of("after"), lost.before)
            .await
            .unwrap(),
        Saved::Kept(_)
    ));
    lost.reconnect().await;
    let revocation = withhold_newest(&lost.harness, lost.tablet.organization(), Some(lost.team));
    let grant = withhold_newest(&lost.harness, lost.tablet.organization(), Some(lost.team));
    assert_eq!(
        lost.tablet.drain(|_| false).await.unwrap(),
        [Drained::Failed { block: lost.note }],
        "an edit judged by the log kept from before the revocation was sent"
    );
    assert_eq!(
        text(&lost.harness.second(&lost.tablet).await, lost.note).await,
        "before"
    );
    grant.serve(&lost.harness);
    revocation.serve(&lost.harness);

    let drained = lost.tablet.drain(|_| false).await.unwrap();
    assert!(
        drained.contains(&Drained::Resealed { block: lost.note }),
        "the edit was not sealed again: {drained:?}"
    );
    assert_eq!(
        lost.tablet.keys().keys_of(lost.team, KeyKind::Scope).len(),
        epochs + 1
    );
    assert!(
        !lost.bob_holds_the_head_s_key().await,
        "the edit was sent under a key the revoked reader holds"
    );
    lost.stop().await;
}
