use super::*;

#[test]
fn a_root_that_keeps_waking_itself_without_answering_stops_at_its_patience() {
    let mut root = root().with_patience(Duration::from_millis(100));
    // The root wakes itself every step and never answers, so it is stepped again and again.
    let drawn = root.frame(FrameRequest { time: SPINS }, Instant::now());
    assert!(
        drawn.is_err(),
        "the frame gave up on the spinning root at its patience"
    );
    assert!(
        root.frame(FrameRequest { time: 1 }, Instant::now() + PATIENT)
            .is_err(),
        "a root that stopped draws nothing again"
    );
}
