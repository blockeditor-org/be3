use super::*;

/// An owner's device revokes a reader and the answer is lost; its offline save is kept. While
/// the server goes on withholding the team's log, each drain tries the edit again, up to
/// `WITHHELD_DRAINS`, and then sets it aside for the person, still never sent under the epoch
/// the reader holds, rather than holding it back for good.
#[tokio::test]
async fn an_edit_whose_log_stays_withheld_after_a_lost_revocation_is_set_aside_after_some_drains()
 {
    let lost = LostRevocation::start().await;
    assert!(matches!(
        lost.tablet
            .save(lost.note, &TextBlock::of("after"), lost.before)
            .await
            .unwrap(),
        Saved::Kept(_)
    ));
    lost.reconnect().await;
    let _revocation = withhold_newest(&lost.harness, lost.tablet.organization(), Some(lost.team));
    let _grant = withhold_newest(&lost.harness, lost.tablet.organization(), Some(lost.team));
    for _ in 1..crate::peer::WITHHELD_DRAINS {
        assert_eq!(
            lost.tablet.drain(|_| false).await.unwrap(),
            [Drained::Failed { block: lost.note }]
        );
        assert!(lost.tablet.unsent().is_empty());
    }
    assert_eq!(
        lost.tablet.drain(|_| false).await.unwrap(),
        [Drained::SetAside {
            block: lost.note,
            why: refs::Aside::KeyOutdated
        }],
        "an edit whose log stays withheld is held back for good"
    );
    assert_eq!(lost.tablet.unsent().len(), 1);
    assert_eq!(
        text(&lost.harness.second(&lost.tablet).await, lost.note).await,
        "before"
    );
    lost.stop().await;
}
