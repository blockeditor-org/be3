use super::*;

/// The host asks the holder to seal, as a flush does, while its edit still waits for its place
/// in the order: the seal is put off, not dropped, and goes once the edit has its place, without
/// waiting for a quiet spell.
#[test]
fn a_seal_asked_for_while_an_edit_waits_for_its_place_goes_once_it_has_one() {
    let mut sim = Sim::<ProseContent>::new(1, prose_module);
    sim.open(0);
    sim.attach(0, 1, true);
    sim.type_at(0, 1, 0, "soup");
    sim.settle();
    sim.advance(crate::QUIET_INTERVAL);
    assert_eq!(prose(&sim.head_content()), "soup");
    let before = sim.publishes;

    sim.stalled_submits = true;
    sim.type_at(0, 1, 4, " and bread");
    sim.settle();
    sim.seal(0);
    assert_eq!(sim.publishes, before, "nothing numbered to seal yet");

    sim.unstall();
    assert_eq!(prose(&sim.head_content()), "soup and bread");
}
