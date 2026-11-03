use super::*;

use beui_host::native::testing::FRAME_INTERVAL;

fn buffer_width(harness: &mut Harness, id: WindowId) -> f64 {
    harness.compositor().server().state.layers(id)[0].buffer.w
}

fn monotonic(harness: &mut Harness) -> Duration {
    let now = harness.context().now();
    harness
        .compositor()
        .server()
        .state
        .presentation
        .monotonic(now)
}

#[test]
fn a_timed_commit_is_applied_on_the_first_frame_at_its_time() {
    let mut harness = Harness::new();
    let (window, id) = harness.open();
    harness.client.attach_unsent(&window, 300, 200);
    harness.settle();
    let timer = harness.client.commit_timer(&window.surface);

    let target = monotonic(&mut harness) + FRAME_INTERVAL * 5 + FRAME_INTERVAL / 2;
    timer.set_timestamp(
        (target.as_secs() >> 32) as u32,
        target.as_secs() as u32,
        target.subsec_nanos(),
    );
    harness.client.attach_unsent(&window, 280, 200);

    let mut applied = None;
    for _ in 0..10 {
        harness.frame(Vec::new());
        if buffer_width(&mut harness, id) == 280.0 {
            applied = Some(monotonic(&mut harness));
            break;
        }
        assert!(
            monotonic(&mut harness) < target,
            "the commit waits only until its time"
        );
    }
    let applied = applied.expect("the commit is applied once its time comes");
    assert!(applied >= target, "it is not applied before its time");
    assert!(
        applied < target + FRAME_INTERVAL,
        "it is applied on the first frame at its time"
    );
}
