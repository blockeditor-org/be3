use super::*;

#[test]
fn a_step_sends_one_edit_at_a_time() {
    let (simulation, left, _right) = simulated("ab");
    typed(&left, 1, "X");
    typed(&left, 2, "Y");
    assert_eq!(simulation.read().unwrap().in_flight(Side::Left), 2);

    simulation.write().unwrap().step(Side::Left, Instant::now());
    assert_eq!(text(&simulation, Side::Right), "aXb");
    assert_eq!(simulation.read().unwrap().in_flight(Side::Left), 1);

    simulation.write().unwrap().step(Side::Left, Instant::now());
    assert_eq!(text(&simulation, Side::Right), "aXYb");
    assert_eq!(simulation.read().unwrap().in_flight(Side::Left), 0);
}
