use super::*;

#[test]
fn paused_typing_on_both_sides_converges_once_sent_both_ways() {
    let (simulation, left, right) = simulated("ab");
    typed(&left, 1, "LL");
    typed(&right, 1, "RR");
    assert_eq!(text(&simulation, Side::Left), "aLLb");
    assert_eq!(text(&simulation, Side::Right), "aRRb");
    assert_eq!(simulation.read().unwrap().in_flight(Side::Left), 1);
    assert_eq!(simulation.read().unwrap().in_flight(Side::Right), 1);

    send(&simulation, Side::Right);
    assert_eq!(text(&simulation, Side::Left), "aRRLLb");
    assert_eq!(simulation.read().unwrap().in_flight(Side::Left), 2);
    send(&simulation, Side::Left);

    assert_eq!(text(&simulation, Side::Right), "aRRLLb");
    assert!(simulation.write().unwrap().take_external(Side::Right));
    assert_eq!(simulation.read().unwrap().in_flight(Side::Left), 0);
    assert_eq!(simulation.read().unwrap().in_flight(Side::Right), 0);

    assert!(right.undo().is_some());
    send(&simulation, Side::Right);
    send(&simulation, Side::Left);
    assert_eq!(text(&simulation, Side::Left), "aLLb");
    assert_eq!(text(&simulation, Side::Right), "aLLb");
}
