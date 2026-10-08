use super::*;

#[test]
fn edits_wait_for_the_latency_before_they_arrive() {
    let (simulation, left, _right) = simulated("");
    let latency = Duration::from_millis(300);
    typed(&left, 0, "hello");
    let sent = Instant::now();
    let due = simulation
        .read()
        .unwrap()
        .next_due(latency)
        .expect("an edit is in flight");
    assert!(due >= sent - Duration::from_millis(50) && due <= sent + latency);

    simulation
        .write()
        .unwrap()
        .deliver(Side::Left, due - Duration::from_millis(1), Some(latency));
    assert_eq!(text(&simulation, Side::Right), "");

    simulation
        .write()
        .unwrap()
        .deliver(Side::Left, due, Some(latency));
    assert_eq!(text(&simulation, Side::Right), "hello");
    assert_eq!(simulation.read().unwrap().next_due(latency), None);
}
