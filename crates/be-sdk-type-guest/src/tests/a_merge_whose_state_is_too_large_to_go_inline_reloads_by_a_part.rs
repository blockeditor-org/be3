use super::*;

/// As `edits_on_their_way_when_another_head_lands_land_on_it`, with the merge's session state
/// past what an entry carries inline: the holder puts it as a part, the reload names the part,
/// and the edits land as they do with the state inline, with no restart.
#[test]
fn a_merge_whose_state_is_too_large_to_go_inline_reloads_by_a_part() {
    crate::replica::LARGEST_INLINE.with(|limit| limit.set(48));
    let mut sim = Sim::<ProseContent>::new(1, prose_module);
    sim.open(0);
    sim.attach(0, 1, true);
    sim.type_at(0, 1, 0, "one\ntwo\nthree");
    sim.advance(crate::QUIET_INTERVAL);

    sim.stalled_submits = true;
    sim.type_at(0, 1, 0, "A");
    sim.type_at(0, 1, 2, "B");
    sim.publish_elsewhere(&prose_of("one\ntwo\nTHREE"));
    sim.unstall();
    sim.advance(crate::QUIET_INTERVAL);

    assert_eq!(sim.devices[0].status.restarts, 0, "no restart");
    assert!(sim.parts_put > 0, "the state went by a part");
    assert_eq!(prose(&sim.content(0)), "AoBne\ntwo\nTHREE");
    assert_eq!(prose(&sim.head_content()), "AoBne\ntwo\nTHREE");
    assert_eq!(sim.typed(0, 1), "AoBne\ntwo\nTHREE");
}
