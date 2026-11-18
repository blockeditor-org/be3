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
    run.painting = true;
    run.pacing.need();
    run.shown
        .keep_left_out(&layout(&[1, 2]), &layout(&[1]), |_| true);
    assert!(run.shown.keeps(ScreenId(2)));

    let mut docks = Docks::default();
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
    assert!(!run.painting);
    assert!(!run.pacing.drawing(), "no frame is in flight");
    assert!(!run.shown.keeps(ScreenId(2)), "the old run's frames go");
    assert!(
        run.tell(telling(true)).is_empty(),
        "nothing but the preferences, sent with its hello, is told before it runs"
    );

    connect(&mut run);
    let told = run.tell(telling(true));
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
