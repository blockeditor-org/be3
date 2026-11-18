use super::*;

#[test]
fn a_frame_is_asked_once_per_host_frame() {
    let mut pacing = Pacing::default();
    let now = Duration::from_millis(100);
    assert_eq!(pacing.ask(true, true, now, 1, false, false), Ask::Nothing);

    pacing.need();
    assert_eq!(pacing.ask(false, true, now, 1, false, false), Ask::Nothing, "it has no surface");
    assert_eq!(pacing.ask(true, true, now, 1, true, false), Ask::Later, "its placer lays out first");
    assert_eq!(pacing.ask(true, true, now, 1, false, true), Ask::AfterInput);
    assert_eq!(pacing.ask(true, true, now, 1, false, false), Ask::Draw);
    assert_eq!(pacing.answered(), (1, 0));
    assert!(pacing.laying_out(1));

    pacing.need();
    assert!(!pacing.will_draw(true, now, 1), "the frame in flight has not timed out");
    pacing.laid_out();
    assert!(pacing.painting());
    pacing.presented(now, false, Some(Duration::from_millis(16)), false);
    assert_eq!(pacing.answered(), (1, 1));
    assert!(pacing.drawing(), "a frame needed while one was in flight is needed again");
    assert_eq!(
        pacing.ask(true, true, now, 1, false, false),
        Ask::Later,
        "an app is asked at most once per host frame"
    );
    assert_eq!(pacing.ask(true, true, now, 2, false, false), Ask::Draw);

    pacing.presented(now, false, Some(Duration::from_millis(16)), true);
    assert_eq!(pacing.answered(), (2, 1), "its input is still waiting");
    assert_eq!(
        pacing.wake_after(now, true),
        Some(Duration::from_millis(16)),
        "it wants its next frame when it said"
    );
    let later = now + Duration::from_millis(16);
    assert_eq!(pacing.ask(true, true, later, 3, false, false), Ask::Draw);
}
