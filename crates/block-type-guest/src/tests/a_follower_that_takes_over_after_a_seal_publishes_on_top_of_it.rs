use super::*;

#[test]
fn a_follower_that_takes_over_after_a_seal_publishes_on_top_of_it() {
    let mut sim = Sim::<CounterContent>::new(2, counter_module);
    for device in 0..2 {
        sim.open(device);
        sim.attach(device, 1, true);
    }
    sim.operate(0, 1, Counter::add(1));
    sim.operate(1, 1, Counter::add(10));
    sim.settle();
    sim.advance(crate::SEAL_INTERVAL);
    let sealed = sim.head.expect("the owner sealed");
    assert_eq!(sim.devices[1].status.head, Some(sealed));

    sim.close(0);
    assert_eq!(
        sim.publishes, 1,
        "closing a clean replica publishes nothing"
    );
    assert!(sim.devices[1].status.owner);
    sim.operate(1, 1, Counter::add(100));
    sim.advance(crate::SEAL_INTERVAL);
    assert_eq!(sim.publishes, 2);
    assert_eq!(
        sim.rejections, 0,
        "taking over at the head demanded a merge"
    );
    let head = sim.head.unwrap();
    assert_eq!(sim.commits[&head].parents, [sealed]);
    assert_eq!(count(&sim.head_content()), 111);
}
