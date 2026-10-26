use super::*;

fn received_from_peers(sim: &mut Sim<ProseContent>) -> usize {
    sim.take_received(0, 1)
        .into_iter()
        .flat_map(|message| match message {
            ToEditor::Operations(operations) => operations,
            _ => Vec::new(),
        })
        .filter(|operation| !operation.mine)
        .count()
}

#[test]
fn typing_behind_an_unconfirmed_edit_goes_out_as_one_edit() {
    let mut sim = Sim::<ProseContent>::new(2, prose_module);
    sim.publish_elsewhere(&prose_of("base"));
    sim.open(0);
    sim.open(1);
    sim.attach(0, 1, true);
    sim.attach(1, 1, true);
    sim.take_received(0, 1);
    let typed = "abcdefghij";
    for at in 0..typed.len() {
        sim.type_at(1, 1, 4 + at, &typed[at..=at]);
    }
    sim.settle();
    let expected = format!("base{typed}");
    assert_eq!(sim.typed(0, 1), expected);
    assert_eq!(sim.typed(1, 1), expected);
    assert!(sim.devices[1].status.clean);
    assert_eq!(
        received_from_peers(&mut sim),
        2,
        "the first letter, then the rest as one edit"
    );

    let paste = "p".repeat(400 * 1024);
    sim.type_at(1, 1, 0, "x");
    for _ in 0..3 {
        let end = sim.typed(1, 1).len();
        sim.type_at(1, 1, end, &paste);
    }
    sim.settle();
    assert_eq!(sim.typed(0, 1), format!("x{expected}{paste}{paste}{paste}"));
    assert_eq!(
        received_from_peers(&mut sim),
        3,
        "the first letter, two pastes merged, then the third on its own"
    );
}
