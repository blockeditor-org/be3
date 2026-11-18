use super::*;

/// Two editors of one block on one device: one types while the other has not heard it, then
/// the other types on what it still shows. Each makes what it inserts under a client of its own,
/// so neither names an item the other made, and both inserts land.
#[test]
fn two_editors_of_one_block_on_one_device_each_land_what_they_type_unheard() {
    let mut sim = Sim::<ProseContent>::new(1, prose_module);
    sim.open(0);
    sim.attach(0, 1, true);
    sim.attach(0, 2, true);
    sim.type_at(0, 1, 0, "start\n");
    sim.settle();

    sim.deafen(0, 1);
    sim.type_at(0, 2, 0, "two ");
    sim.settle();
    sim.type_at(0, 1, 5, "!");
    sim.settle();
    sim.hear(0, 1);

    assert_eq!(prose(&sim.content(0)), "two start!\n");
    assert_eq!(sim.typed(0, 1), "two start!\n");
    assert_eq!(sim.typed(0, 2), "two start!\n");
}
