use super::*;

#[test]
fn a_late_joiner_places_an_insert_after_an_item_removed_before_it_joined() {
    let mut sim = Sim::<ChecklistContent>::new(2, checklist_module);
    sim.open(0);
    sim.attach(0, 1, true);
    let mut removed = ObjectId::ROOT;
    for text in ["a", "b", "c"] {
        let (id, edit) = Checklist::add(text);
        if text == "b" {
            removed = id;
        }
        sim.operate(0, 1, edit);
    }
    sim.operate(0, 1, Checklist::remove(removed));
    sim.advance(crate::SEAL_INTERVAL);
    sim.open(1);
    sim.attach(1, 1, true);
    assert_eq!(texts(sim.mirror(1, 1)), ["a", "c"]);
    let (_, insert) = Checklist::ITEMS.insert(
        ObjectId::ROOT,
        Anchor::After(removed),
        &ChecklistItem {
            text: "d".into(),
            done: false,
        },
    );
    sim.operate(0, 1, insert.into());
    sim.settle();
    for device in 0..2 {
        assert_eq!(texts(&sim.content(device)), ["a", "d", "c"]);
        assert_eq!(texts(sim.mirror(device, 1)), ["a", "d", "c"]);
    }
}
