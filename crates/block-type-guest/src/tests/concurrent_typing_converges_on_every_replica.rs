use super::*;

#[test]
fn concurrent_typing_converges_on_every_replica() {
    let mut sim = Sim::<ProseContent>::new(3, prose_module);
    sim.publish_elsewhere(&prose_of("|"));
    for device in 0..3 {
        sim.open(device);
        sim.attach(device, 1, true);
    }
    let runs = ["abc", "123", "xyz"];
    for at in 0..3 {
        for (device, run) in runs.iter().enumerate() {
            let typed = &run[at..=at];
            let position = sim.typed(device, 1).find('|').unwrap_or_default();
            sim.type_at(device, 1, position, typed);
        }
    }
    sim.settle();
    let first = sim.typed(0, 1);
    for run in runs {
        assert!(
            run.chars().all(|letter| first.contains(letter)),
            "{run} was lost from {first:?}"
        );
    }
    for device in 0..3 {
        assert_eq!(sim.typed(device, 1), first);
        assert_eq!(prose(&sim.content(device)), first);
    }
    sim.advance(crate::SEAL_INTERVAL);
    assert_eq!(prose(&sim.head_content()), first);
    for device in 0..3 {
        assert!(sim.devices[device].status.clean);
    }
}
