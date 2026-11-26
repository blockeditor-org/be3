use super::*;

/// An owner's device revokes a reader; the server takes it, and the connection drops before its
/// answer arrives. Saved before the device connects again, the edit is kept for the outbox,
/// sealed under the epoch at hand, as a log read cannot reach the server; but the team's log
/// stays unsure until a read of it answers, so the drain once connected judges the edit by no
/// log kept from before: while the server withholds the log it tries the edit again later, and
/// once a read answers it sees the revocation and seals the edit again under a new epoch rather
/// than send it under the one the revoked reader holds.
#[tokio::test]
async fn an_owner_s_offline_save_after_its_revocation_s_answer_was_lost_is_never_sent_under_the_old_epoch()
 {
    let lost = LostRevocation::start().await;
    let epochs = lost.tablet.keys().keys_of(lost.team, KeyKind::Scope).len();
    assert!(matches!(
        lost.tablet
            .save(lost.note, &TextBlock::of("after"), lost.before)
            .await
            .unwrap(),
        Saved::Kept(_)
    ));
    lost.reconnect().await;
    // The server then withholds the team's log from it (served short of what the device kept):
    // the read answers nothing, so the log stays unsure, and the edit is neither sent under the
    // kept log's judgement nor set aside, but tried again at the next drain.
    let revocation = withhold_newest(&lost.harness, lost.tablet.organization(), Some(lost.team));
    let grant = withhold_newest(&lost.harness, lost.tablet.organization(), Some(lost.team));
    assert_eq!(
        lost.tablet.drain(|_| false).await.unwrap(),
        [Drained::Failed { block: lost.note }],
        "an edit judged while the log is withheld was not tried again later"
    );
    assert!(lost.tablet.unsent().is_empty());
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
        epochs + 1,
        "the drain gives the scope a new epoch"
    );
    assert!(
        !lost.bob_holds_the_head_s_key().await,
        "the edit was sent under a key the revoked reader holds"
    );
    assert_eq!(
        text(&lost.harness.second(&lost.tablet).await, lost.note).await,
        "after"
    );
    lost.stop().await;
}
