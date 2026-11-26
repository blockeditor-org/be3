use super::*;

/// An owner's device revokes a reader; the server takes it, and the connection drops before its
/// answer arrives. Connected again, the device's next save reads the team's log again rather
/// than seal by the one it checked before the revocation, so it gives the scope a new epoch
/// the revoked reader never gets.
#[tokio::test]
async fn an_owner_s_save_after_its_revocation_s_answer_was_lost_reads_the_log_again() {
    let lost = LostRevocation::start().await;
    let epochs = lost.tablet.keys().keys_of(lost.team, KeyKind::Scope).len();
    lost.reconnect().await;
    assert!(
        lost.tablet
            .save(lost.note, &TextBlock::of("after"), lost.before)
            .await
            .unwrap()
            .published()
            .is_some()
    );
    assert_eq!(
        lost.tablet.keys().keys_of(lost.team, KeyKind::Scope).len(),
        epochs + 1,
        "the save gives the scope a new epoch"
    );
    assert!(
        !lost.bob_holds_the_head_s_key().await,
        "the save was sealed under a key the revoked reader holds"
    );
    lost.stop().await;
}
