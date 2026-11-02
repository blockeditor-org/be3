use super::*;

#[test]
fn a_follower_that_missed_messages_catches_up_once_resynchronized() {
    let mut sim = Sim::<ChecklistContent>::new(2, checklist_module);
    sim.open(0);
    sim.open(1);
    sim.attach(0, 1, true);
    sim.attach(1, 1, true);
    sim.operate(0, 1, add("start"));
    sim.settle();
    assert_eq!(texts(sim.mirror(1, 1)), ["start"]);

    sim.deaf.insert(1);
    for text in ["a", "b", "c"] {
        sim.operate(0, 1, add(text));
    }
    sim.advance(crate::SEAL_INTERVAL);
    for text in ["d", "e"] {
        sim.operate(0, 1, add(text));
    }
    sim.settle();
    assert_eq!(texts(sim.mirror(1, 1)), ["start"]);

    sim.deaf.clear();
    sim.resynchronize(1);
    let owner = texts(&sim.content(0));
    assert_eq!(owner, ["start", "a", "b", "c", "d", "e"]);
    assert_eq!(texts(&sim.content(1)), owner);
    assert_eq!(texts(sim.mirror(1, 1)), owner);
    sim.operate(1, 1, add("f"));
    sim.settle();
    assert_eq!(
        texts(sim.mirror(0, 1)).last().map(String::as_str),
        Some("f")
    );
}
