use super::*;

#[test]
fn a_followers_edit_too_large_to_relay_is_lacking_until_the_owner_accepts_it() {
    let mut sim = Sim::<ProseContent>::new(2, prose_module);
    sim.publish_elsewhere(&prose_of("start\n"));
    sim.open(0);
    sim.open(1);
    sim.attach(0, 1, true);
    sim.attach(1, 1, true);
    sim.send_from_editor(1, 1, FromEditor::FollowLacking);
    sim.settle();

    sim.stalled = true;
    sim.type_at(1, 1, 6, &"a line of pasted text\n".repeat(60_000));
    sim.settle();
    assert_eq!(
        sim.editors[&(1, 1)].lacking().len(),
        1,
        "the owner has not been sent the edit while its part is not put"
    );
    sim.unstall();
    sim.settle();
    sim.advance(crate::QUIET_INTERVAL);
    assert!(
        sim.editors[&(1, 1)].lacking().is_empty(),
        "the owner accepted the edit sent by its part, and the server holds it"
    );
}
