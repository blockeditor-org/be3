use super::*;

#[test]
fn a_restart_forgets_what_the_old_run_held_and_tells_the_new_one_again() {
    let mut run = running();
    let told = run.tell(telling(true));
    assert!(matches!(
        told.as_slice(),
        [
            AppCall::Fonts(_),
            AppCall::Describe(true),
            AppCall::UtcOffset(60)
        ]
    ));
    assert!(
        run.tell(telling(true)).is_empty(),
        "what has not changed is not told again"
    );
    run.queued.push(Outbound::App(AppCall::Paintings));
    run.holds.hold(());
    run.screens = vec![screen_request(ScreenId(1))];
    run.painting = true;
    run.pacing.need();
    run.shown
        .keep_left_out(&layout(&[1, 2]), &layout(&[1]), |_| true);
    assert!(run.shown.keeps(ScreenId(2)));

    let mut docks = Docks::default();
    docks.register(EditorInstanceId(1), "demo", None);
    docks.register(EditorInstanceId(2), "other", Some(uuid::Uuid::from_u128(2)));
    docks.placed(
        EditorInstanceId(1),
        &[crate::docks::DockedChild {
            child: be_sdk_api::ChildId(10),
            tab: 1,
            block: uuid::Uuid::from_u128(2),
        }],
        None,
    );
    assert!(docks.docked(EditorInstanceId(2)));
    let mut views = described();
    run.restart(
        Session::new("Test", None),
        Preferences::default(),
        "demo",
        &mut docks,
        &mut views,
    );
    assert_eq!(*run.session.state(), crate::session::SessionState::Idle);
    assert!(run.queued.is_empty());
    assert!(run.holds.is_empty(), "the answers held go");
    assert!(run.screens.is_empty(), "the new run is sent its screens again");
    assert!(
        !docks.docked(EditorInstanceId(2)),
        "what the app's instances placed in their docks goes"
    );
    assert!(!run.painting);
    assert!(!run.pacing.drawing(), "no frame is in flight");
    assert!(!run.shown.keeps(ScreenId(2)), "the old run's frames go");
    assert!(
        run.tell(telling(true)).is_empty(),
        "nothing but the preferences, sent with its hello, is told before it runs"
    );
    let changed = Preferences {
        theme: be_sdk_api::ThemeKind::Light,
        ..Preferences::default()
    };
    assert_ne!(changed, Preferences::default());
    let told = run.tell(Telling {
        preferences: changed,
        ..telling(true)
    });
    assert!(
        matches!(told.as_slice(), [AppCall::Preferences(preferences)] if *preferences == changed),
        "preferences that change before it runs are told at once, the rest wait"
    );
    assert_eq!(run.preferences(), changed, "its hello carries them");

    connect(&mut run);
    let told = run.tell(Telling {
        preferences: changed,
        ..telling(true)
    });
    assert!(
        matches!(
            told.as_slice(),
            [
                AppCall::Fonts(_),
                AppCall::Describe(true),
                AppCall::UtcOffset(60)
            ]
        ),
        "the new run is told again once it runs"
    );
}
