use super::*;

/// Two edits typed behind one on its way go out as one submission, and the replica is opened
/// again (as after a crash) before the server numbers it. Given both edits again with that
/// submission, it submits them under its id once both are given, with the announcement of each,
/// so the host sends both once the server holds the submission.
#[test]
fn edits_given_again_with_the_submission_that_held_them_carry_their_announcements() {
    let mut sim = Sim::<AnnouncedCounter>::new(2, announced_counter_module);
    for device in 0..2 {
        sim.open(device);
        sim.attach(device, 1, true);
    }
    sim.stalled_submits = true;
    for by in [1, 10, 100] {
        sim.operate(1, 1, Counter::add(by));
    }
    sim.settle();
    let Some((
        _,
        Call::Submit {
            submission: first,
            reply,
            ..
        },
    )) = sim.held.pop()
    else {
        panic!("the first edit was submitted");
    };
    sim.number(1, first);
    sim.to(1, |host| {
        reply.send(host, &be_sdk_type_api::host::Submitted::Held)
    });
    sim.settle();
    let Some((
        _,
        Call::Submit {
            submission,
            announcements,
            ..
        },
    )) = sim.held.pop()
    else {
        panic!("the two edits behind it were submitted");
    };
    assert_eq!(announcements.len(), 2, "each edit announces");
    let through = Submitted::decode(&submission.payload)
        .expect("a submission decodes")
        .through;
    assert_eq!(through, 3, "one submission holds both");
    let given = sim.devices[1].status.journaled_taken as usize;
    assert_eq!(given, 1, "the order holds the first edit");
    sim.kept_submitted = through;
    sim.kept_submissions = vec![be_sdk_type_api::KeptSubmission {
        counter: submission.counter,
        through,
        payload: submission.payload.clone(),
    }];
    sim.reopen(1);
    for by in [10, 100] {
        sim.send_from_editor(
            1,
            1,
            FromEditor::Operate(AnnouncedCounter::encode_operation(&Counter::add(by))),
        );
        sim.settle();
    }
    let again: Vec<_> = std::mem::take(&mut sim.held)
        .into_iter()
        .filter_map(|(_, call)| match call {
            Call::Submit {
                submission,
                announcements,
                reply,
                ..
            } => Some((submission, announcements, reply)),
            _ => None,
        })
        .collect();
    let [(resubmitted, announcements, reply)] = <[_; 1]>::try_from(again)
        .unwrap_or_else(|again| panic!("{} submissions, not one", again.len()));
    assert_eq!(
        resubmitted.counter, submission.counter,
        "it went under its id"
    );
    assert_eq!(resubmitted.payload, submission.payload, "it went as it was");
    assert_eq!(
        announcements.len(),
        2,
        "it carries the announcement of each edit it holds"
    );
    sim.stalled_submits = false;
    sim.number(1, resubmitted);
    sim.to(1, |host| {
        reply.send(host, &be_sdk_type_api::host::Submitted::Held)
    });
    // The first copy reaches the server late: its id is held.
    sim.number(1, submission);
    sim.settle();
    for _ in 0..3 {
        sim.advance(crate::QUIET_INTERVAL);
        sim.settle();
    }
    assert_eq!(count(&sim.head_content().0), 111, "each edit landed once");
    assert!(sim.devices[1].status.clean, "the edits settled");
    assert!(sim.errors.is_empty(), "{:?}", sim.errors);
}
