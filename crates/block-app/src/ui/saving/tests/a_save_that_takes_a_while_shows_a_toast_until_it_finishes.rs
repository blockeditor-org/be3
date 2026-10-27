use super::*;

#[test]
fn a_save_that_takes_a_while_shows_a_toast_until_it_finishes() {
    let mut app = Saving::start();
    let short = SLOW_SAVE - Duration::from_secs(1);
    assert!(
        !app.after(short, saving(false, 0)),
        "a save that finishes quickly shows nothing"
    );
    assert!(
        !app.after(short, saving(false, 1)),
        "nor does one that keeps landing commits while edits keep coming"
    );
    assert!(
        app.after(Duration::from_secs(2), saving(false, 1)),
        "a save that makes no progress for a while shows a toast"
    );
    assert!(
        app.after(TOAST_DURATION, saving(false, 2)),
        "which stays up past the usual toast duration while saving goes on"
    );
    let output = app.stepped.frame(Vec::new());
    let painted = beui::native::painting::painting(
        output.main(),
        vec2(1100.0, 720.0),
        1.0,
        output.main().window.appearance.clear_color,
        beui::native::painting::Punches::Shown,
    )
    .unwrap();
    let snapshot = paint_snapshot::Snapshot::of_painting(painted).unwrap();
    paint_snapshot::assert_accepted(
        "a_save_that_takes_a_while_shows_a_toast_until_it_finishes",
        &snapshot,
        "./scripts/verify",
    );
    assert!(
        !app.after(Duration::from_millis(300), saving(true, 3)),
        "and goes away once everything is saved"
    );
}
