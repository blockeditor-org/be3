use super::*;

/// A merge's reload the server refuses is let go rather than waited on, which would hold every
/// seal back for good: the merge is made again on its timer, as one that failed is, and once
/// its reload goes the edits on their way land and the holder seals again.
#[test]
fn a_reload_the_server_refuses_is_made_again_and_seals_go_on() {
    let mut sim = Sim::<ProseContent>::new(1, prose_module);
    sim.open(0);
    sim.attach(0, 1, true);
    sim.type_at(0, 1, 0, "one\ntwo\nthree");
    sim.advance(crate::QUIET_INTERVAL);

    sim.stalled_submits = true;
    sim.type_at(0, 1, 0, "A");
    sim.type_at(0, 1, 2, "B");
    // Every edit goes; a reload carrying a session state does not.
    sim.largest_submission = 60;
    sim.publish_elsewhere(&prose_of("one\ntwo\nTHREE"));
    sim.unstall();
    assert!(
        !sim.devices[0].status.clean && !sim.devices[0].status.owes_seal,
        "the merge waits for its timer, owing no seal meanwhile: {:?}",
        sim.devices[0].status
    );

    sim.largest_submission = usize::MAX;
    sim.advance(crate::replica::MERGE_RETRY_CEILING);
    sim.advance(crate::QUIET_INTERVAL);
    assert_eq!(prose(&sim.content(0)), "AoBne\ntwo\nTHREE");
    sim.type_at(0, 1, 0, "C");
    sim.advance(crate::QUIET_INTERVAL);
    assert_eq!(
        prose(&sim.head_content()),
        "CAoBne\ntwo\nTHREE",
        "the holder seals again"
    );
}
