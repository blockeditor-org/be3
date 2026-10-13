use super::*;

#[test]
fn clicking_a_frame_in_the_profiler_keeps_showing_that_frame() {
    let HelloColumn { document, .. } = hello_column();
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.toggle_inspector();
    harness.click(harness.performance_tab_center());
    harness.frame(Vec::new());

    harness.click(harness.profiler_point(0.999));
    harness.frame(vec![Event::PointerMoved(harness.performance_tab_center())]);
    let picked = harness.profiled_frame();
    assert!(picked.starts_with("Frame "), "{picked}");

    for _ in 0..3 {
        harness.frame(Vec::new());
    }
    assert_eq!(
        harness.profiled_frame(),
        picked,
        "a picked frame stays shown while newer ones arrive"
    );

    harness.key(Key::ArrowLeft, Modifiers::default());
    harness.frame(Vec::new());
    let number = |text: &str| -> u64 {
        text.trim_start_matches("Frame ")
            .split(':')
            .next()
            .and_then(|number| number.parse().ok())
            .expect("the frame summary starts with its number")
    };
    assert_eq!(
        number(&harness.profiled_frame()),
        number(&picked) - 1,
        "the left arrow steps to the frame before"
    );
}
