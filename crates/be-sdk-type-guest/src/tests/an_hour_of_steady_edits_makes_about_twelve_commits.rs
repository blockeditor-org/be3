use super::*;

#[test]
fn an_hour_of_steady_edits_makes_about_twelve_commits() {
    let mut sim = Sim::<CounterContent>::new(2, counter_module);
    sim.open(0);
    sim.open(1);
    sim.attach(0, 1, true);
    let hour = 60 * 60 * 1000;
    let mut edits = 0;
    while sim.now < 1_000 + hour {
        sim.operate(0, 1, Counter::add(1));
        edits += 1;
        sim.advance(5_000);
    }
    assert!(
        (11..=13).contains(&sim.publishes),
        "{} commits for an hour of steady edits",
        sim.publishes
    );
    sim.advance(crate::QUIET_INTERVAL);
    assert_eq!(count(&sim.head_content()), edits);
    assert_eq!(count(&sim.content(1)), edits);
}
