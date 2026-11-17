use super::*;

/// Two edits typed behind one on its way go out as one submission, and the replica is opened
/// again (as after a crash) before the server numbers it. Given both edits again with that
/// submission, the replica waits for the second before it submits them as exactly that, under
/// its id: the first copy reaching the server late lands nothing twice. When the first copy is
/// numbered while only the first edit was given again, the replica finds both in the order and
/// sends neither.
#[test]
fn two_edits_one_submission_held_go_again_as_it_once_both_are_given_again() {
    for late in [true, false] {
        let mut sim = Sim::<CounterContent>::new(2, counter_module);
        for device in 0..2 {
            sim.open(device);
            sim.attach(device, 1, true);
        }
        sim.stalled_submits = true;
        sim.operate(1, 1, Counter::add(1));
        sim.operate(1, 1, Counter::add(10));
        sim.operate(1, 1, Counter::add(100));
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
        let Some((_, Call::Submit { submission, .. })) = sim.held.pop() else {
            panic!("the two edits behind it were submitted");
        };
        sim.stalled_submits = false;
        let through = Submitted::decode(&submission.payload)
            .expect("a submission decodes")
            .through;
        assert_eq!(through, 3, "one submission holds both");
        let given = sim.devices[1].status.journaled_taken as usize;
        sim.kept_submitted = through;
        sim.kept_submissions = vec![be_sdk_type_api::KeptSubmission {
            counter: submission.counter,
            through,
            payload: submission.payload.clone(),
        }];
        sim.reopen(1);
        let edits: Vec<i64> = [1, 10, 100].into_iter().skip(given).collect();
        let (last, before) = edits
            .split_last()
            .expect("the edits behind are given again");
        for by in before {
            sim.send_from_editor(
                1,
                1,
                FromEditor::Operate {
                    gesture: None,
                    operation: CounterContent::encode_operation(&Counter::add(*by)),
                },
            );
        }
        sim.settle();
        if !late {
            sim.number(1, submission.clone());
            sim.settle();
        }
        sim.send_from_editor(
            1,
            1,
            FromEditor::Operate {
                gesture: None,
                operation: CounterContent::encode_operation(&Counter::add(*last)),
            },
        );
        sim.settle();
        if late {
            sim.number(1, submission);
            sim.settle();
        }
        for _ in 0..3 {
            sim.advance(crate::QUIET_INTERVAL);
            sim.settle();
        }
        assert_eq!(
            count(&sim.head_content()),
            111,
            "each edit landed once (late: {late})"
        );
        for device in 0..2 {
            assert_eq!(count(&sim.content(device)), 111, "device {device}");
        }
        assert!(sim.devices[1].status.clean, "the edits settled");
        assert!(sim.errors.is_empty(), "{:?}", sim.errors);
    }
}
