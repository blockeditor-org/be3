use super::*;

/// As `an_edit_given_again_with_the_submission_that_held_it_lands_once_when_both_reach_the_server`
/// for an edit too large to relay, which goes by the part it was put as: given again, it is put
/// again, the same bytes under the same key make the same part, and it goes as the kept
/// submission under its id, so the first copy reaching the server late lands nothing more.
#[test]
fn an_edit_by_its_part_given_again_with_the_submission_that_held_it_lands_once() {
    crate::replica::LARGEST_INLINE.with(|largest| largest.set(1));
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
    let submitted = Submitted::decode(&submission.payload).expect("a submission decodes");
    let through = submitted.through;
    let crate::wire::Entry::Operation(be_block_session::Payload::Part(part)) = &submitted.entry
    else {
        panic!("the edit went by its part: {submitted:?}");
    };
    let part = *part;
    let typed = sim
        .vault
        .read_part(part, 0, part.length())
        .expect("the part was put");
    assert_eq!(typed, CounterContent::encode_operation(&Counter::add(10)));
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
            operation: typed,
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
