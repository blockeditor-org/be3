use super::*;

#[test]
fn pausing_the_profiler_stops_its_frames_moving() {
    let HelloColumn { document, .. } = hello_column();
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.toggle_inspector();
    harness.click(harness.performance_tab_center());
    harness.frame(Vec::new());

    harness.click(harness.inspector_center("inspector.performance.pause"));
    harness.frame(Vec::new());
    let paused = harness.profiled_frame();
    let samples = harness.document().performance().samples;
    for _ in 0..3 {
        harness.frame(Vec::new());
    }
    assert!(harness.document().performance().samples > samples);
    assert_eq!(
        harness.profiled_frame(),
        paused,
        "a paused profiler keeps the frames it had"
    );

    harness.click(harness.inspector_center("inspector.performance.pause"));
    harness.frame(Vec::new());
    assert_ne!(
        harness.profiled_frame(),
        paused,
        "resuming shows new frames again"
    );
}
