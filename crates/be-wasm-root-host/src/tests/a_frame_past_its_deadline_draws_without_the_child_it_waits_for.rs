use super::*;

#[test]
fn a_frame_past_its_deadline_draws_without_the_child_it_waits_for() {
    let mut root = root();
    // The child hangs, so it never answers: the frame ends at its deadline, which has passed.
    let done = root
        .frame(FrameRequest { time: HANGS_CHILD }, Instant::now())
        .expect("the root draws");
    assert_eq!(
        shown(&done),
        vec![ROOT_LAYER],
        "the frame drew what it had at its deadline: the root alone"
    );
}
