use super::*;

#[test]
fn an_app_owes_its_root_only_what_is_due_now() {
    let idle = Pacing::default();
    for starting in [
        SessionState::Idle,
        SessionState::Starting,
        SessionState::ShuttingDown,
    ] {
        assert_eq!(
            wants(&starting, true, &idle, NOW),
            Wants {
                next: Some(NOW + FRAME_TIMEOUT),
                owed: true,
            },
            "an app starting or going owes its hello or its going, by its frame's timeout"
        );
    }
    assert_eq!(
        wants(&SessionState::Closed, true, &idle, NOW),
        Wants::default(),
        "an app that closed wants nothing, though no one removed it"
    );

    let mut needed = Pacing::default();
    needed.need();
    assert_eq!(
        wants(&SessionState::Running, false, &needed, NOW),
        Wants::default(),
        "an app with no surface is never asked for a frame, so it wants and owes none"
    );
    assert_eq!(
        wants(&SessionState::Running, true, &needed, NOW),
        Wants {
            next: Some(NOW),
            owed: true,
        },
        "a frame needed and not asked for is owed at once"
    );
    asked(&mut needed);
    assert_eq!(
        wants(&SessionState::Running, true, &needed, NOW),
        Wants {
            next: Some(NOW + FRAME_TIMEOUT),
            owed: true,
        },
        "a frame in flight is owed, its present waking the root by its timeout"
    );

    let mut animating = Pacing::default();
    animating.need();
    asked(&mut animating);
    animating.presented(NOW, true, None, false);
    assert_eq!(
        wants(&SessionState::Running, true, &animating, NOW),
        Wants {
            next: Some(NOW),
            owed: false,
        },
        "an animating app wants its next frame at once but owes none, so its root settles"
    );

    let mut later = Pacing::default();
    later.need();
    asked(&mut later);
    let after = Duration::from_millis(300);
    later.presented(NOW, false, Some(after), false);
    assert_eq!(
        wants(&SessionState::Running, true, &later, NOW),
        Wants {
            next: Some(NOW + after),
            owed: false,
        },
        "an app that asked to draw again later wants that frame but owes nothing now"
    );
}
