use super::*;

#[test]
fn an_edit_too_large_to_relay_is_lacking_until_its_commit_is_published() {
    let mut sim = Sim::<ProseContent>::new(1, prose_module);
    sim.publish_elsewhere(&prose_of("start\n"));
    sim.open(0);
    sim.attach(0, 1, true);
    sim.send_from_editor(0, 1, FromEditor::FollowLacking);
    sim.settle();

    sim.stalled = true;
    sim.type_at(0, 1, 6, &"a line of pasted text\n".repeat(60_000));
    sim.settle();
    assert_eq!(
        sim.editors[&(0, 1)].lacking().len(),
        1,
        "the server lacks the large edit while its commit is unanswered"
    );
    sim.unstall();
    assert!(sim.editors[&(0, 1)].lacking().is_empty());
}
