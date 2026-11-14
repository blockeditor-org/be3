use super::arrivals::*;
use super::*;

/// A note that only just fits inline would not once stamped: the owner sequences it as it came,
/// its arrival unknown, rather than refusing it or holding it back.
#[test]
fn a_stamp_that_would_not_fit_inline_is_left_off() {
    let mut sim = Sim::<Arrivals>::new(2, arrivals_module);
    for device in 0..2 {
        sim.open(device);
        sim.attach(device, 1, true);
    }
    let largest = crate::replica::LARGEST_RELAYED_OPERATION;
    let mut note = Note::sized(largest);
    while Arrivals::encode_operation(&note).len() > largest {
        note.padding.pop();
    }
    assert_eq!(Arrivals::encode_operation(&note).len(), largest);

    sim.operate(1, 1, note);
    sim.operate(1, 1, Note::default());
    sim.settle();
    let now = sim.now as i64;
    for device in 0..2 {
        assert_eq!(
            entries(&mut sim, device),
            vec![None, Some(now)],
            "device {device}"
        );
    }
    assert!(sim.errors.is_empty(), "{:?}", sim.errors);
}
