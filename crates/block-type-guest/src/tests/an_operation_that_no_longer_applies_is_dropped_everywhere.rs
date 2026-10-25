use super::*;

#[test]
fn an_operation_that_no_longer_applies_is_dropped_everywhere() {
    let mut sim = Sim::<Tally>::new(2, tally_module);
    for device in 0..2 {
        sim.open(device);
        sim.attach(device, 1, true);
    }
    sim.operate(
        0,
        1,
        Mark {
            value: 1,
            alone: false,
        },
    );
    sim.operate(
        1,
        1,
        Mark {
            value: 2,
            alone: true,
        },
    );
    sim.settle();
    for device in 0..2 {
        assert_eq!(sim.content(device), Tally(vec![1]));
        assert_eq!(sim.mirror(device, 1), &Tally(vec![1]));
    }
    assert!(
        sim.devices[1].status.clean,
        "the dropped edit was still waiting to be confirmed"
    );
    sim.operate(
        1,
        1,
        Mark {
            value: 3,
            alone: true,
        },
    );
    sim.settle();
    for device in 0..2 {
        assert_eq!(sim.content(device), Tally(vec![1, 3]));
    }
}
