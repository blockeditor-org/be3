use super::*;

/// A replica submits an edit, and is opened again (as after a crash) before the server numbers
/// it. The host gives the edit again with the submission that held it: the replica submits it as
/// exactly that, under the same id, so when the first copy reaches the server too, the server
/// holds its id already and the edit lands once.
#[test]
fn an_edit_given_again_with_the_submission_that_held_it_lands_once_when_both_reach_the_server() {
    let mut sim = Sim::<CounterContent>::new(2, counter_module);
    for device in 0..2 {
        sim.open(device);
        sim.attach(device, 1, true);
    }
    sim.operate(1, 1, Counter::add(1));
    sim.settle();

    sim.stalled_submits = true;
    sim.operate(1, 1, Counter::add(10));
    sim.settle();
    let Some((_, Call::Submit { submission, .. })) = sim.held.pop() else {
        panic!("the edit was submitted");
    };
    sim.stalled_submits = false;
    let through = Submitted::decode(&submission.payload)
        .expect("a submission decodes")
        .through;
    sim.kept_submitted = through;
    sim.kept_submissions = vec![be_sdk_type_api::KeptSubmission {
        counter: submission.counter,
        through,
        payload: submission.payload.clone(),
    }];
    sim.reopen(1);
    sim.send_from_editor(
        1,
        1,
        FromEditor::Operate {
            gesture: None,
            operation: CounterContent::encode_operation(&Counter::add(10)),
        },
    );
    sim.settle();
    // The first copy reaches the server after the one given again.
    sim.number(1, submission);
    sim.settle();
    for _ in 0..3 {
        sim.advance(crate::QUIET_INTERVAL);
        sim.settle();
    }
    assert_eq!(count(&sim.head_content()), 11, "the edit landed once");
    for device in 0..2 {
        assert_eq!(count(&sim.content(device)), 11, "device {device}");
    }
    assert!(sim.devices[1].status.clean, "the edit settled");
    assert!(sim.errors.is_empty(), "{:?}", sim.errors);
}
