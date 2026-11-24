use super::*;

#[test]
fn a_frame_past_its_deadline_draws_without_the_child_it_waits_for() {
    let mut root = root().with_patience(PATIENT);
    // The child hangs, so it never answers: the frame ends at its deadline, which has passed,
    // without waiting on the child even to start.
    let started = Instant::now();
    let done = root
        .frame(
            FrameRequest {
                time: HANGS_CHILD,
                wall_clock: 0,
                utc_offset: 0,
                window: window(),
            },
            started,
        )
        .expect("the root draws");
    assert!(
        started.elapsed() < PATIENT / 6,
        "the frame ended at its deadline, well inside the root's patience"
    );
    assert_eq!(
        shown(&done),
        vec![ROOT_LAYER],
        "the frame drew what it had at its deadline: the root alone"
    );
}
