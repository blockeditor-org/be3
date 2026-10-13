use super::*;

#[test]
fn held_typing_merges_into_one_edit() {
    let (simulation, left, right) = simulated("ab");
    simulation.write().unwrap().hold(true, Instant::now());
    typed(&right, 1, "x");
    typed(&right, 2, "y");
    typed(&right, 3, "z");
    typed(&left, 0, "L");
    typed(&left, 1, "M");
    assert_eq!(simulation.read().unwrap().in_flight(Side::Right), 1);
    assert_eq!(simulation.read().unwrap().in_flight(Side::Left), 1);

    let caret = {
        let read = right.read().unwrap();
        Caret {
            anchor: Position::at(&*read, 2),
            focus: Position::at(&*read, 2),
        }
    };
    for _ in 0..3 {
        simulation
            .write()
            .unwrap()
            .publish(Side::Right, caret, Instant::now());
    }
    assert_eq!(simulation.write().unwrap().up.len(), 2);

    send(&simulation, Side::Right);
    send(&simulation, Side::Left);
    assert_eq!(text(&simulation, Side::Left), "LMaxyzb");
    assert_eq!(text(&simulation, Side::Right), "LMaxyzb");
    assert!(simulation.read().unwrap().peers[1].pending.is_empty());
    assert_eq!(simulation.read().unwrap().seen(Side::Left), Some(caret));
}
