use super::*;

/// A replica is opened again with a submission an earlier opening made that does not hold the
/// edits given again under it, as one carried across a restart of the order holds them moved
/// and numbered otherwise: the edits it would cover do not make it again exactly, so they go as
/// a new submission rather than under its id, and each lands.
#[test]
fn edits_given_again_with_a_submission_that_holds_other_edits_go_anew() {
    let mut sim = Sim::<CounterContent>::new(2, counter_module);
    for device in 0..2 {
        sim.open(device);
        sim.attach(device, 1, true);
    }
    sim.stalled_submits = true;
    sim.operate(1, 1, Counter::add(1));
    sim.settle();
    let Some((_, Call::Submit { submission, .. })) = sim.held.pop() else {
        panic!("the edit was submitted");
    };
    sim.stalled_submits = false;
    // Its payload holds the first edit alone, and it claims the second too.
    sim.kept_submitted = 2;
    sim.kept_submissions = vec![be_sdk_type_api::KeptSubmission {
        counter: submission.counter,
        through: 2,
        payload: submission.payload,
    }];
    sim.reopen(1);
    for by in [1, 10] {
        sim.send_from_editor(
            1,
            1,
            FromEditor::Operate {
                gesture: None,
                operation: CounterContent::encode_operation(&Counter::add(by)),
            },
        );
    }
    sim.settle();
    for _ in 0..3 {
        sim.advance(crate::QUIET_INTERVAL);
        sim.settle();
    }
    assert_eq!(count(&sim.head_content()), 11, "each edit landed");
    for device in 0..2 {
        assert_eq!(count(&sim.content(device)), 11, "device {device}");
    }
    assert!(sim.errors.is_empty(), "{:?}", sim.errors);
}
