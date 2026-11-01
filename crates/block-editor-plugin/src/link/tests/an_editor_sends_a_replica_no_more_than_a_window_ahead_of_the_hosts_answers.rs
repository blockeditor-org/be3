use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use block_plugin_api::{
    embed::blocks::Call,
    ops::{
        Connection, Incoming, OpId, Outcome, STREAM_WINDOW, Side,
        guest::{Executor, Serve},
    },
};

use super::*;

const INSTANCE: EditorInstanceId = EditorInstanceId(1);

fn sent(executor: &Executor, host: &mut Connection) -> Vec<(OpId, u8)> {
    let mut items = Vec::new();
    executor
        .step(host.take_outbound(), &mut items)
        .expect("the host's frames decode");
    for item in items {
        host.receive(&item).expect("the guest's frames decode");
    }
    let mut sent = Vec::new();
    while let Some(event) = host.next_event() {
        let Incoming::Started(started) = event else {
            continue;
        };
        if let Some(Call::ReplicaSend { frame, reply, .. }) = Call::accept(host, started) {
            let Some(FromEditor::Operate(operation)) = FromEditor::from_frame(&frame) else {
                panic!("the editor sent something other than its operation");
            };
            sent.push((reply.into_op(), operation[0]));
        }
    }
    sent
}

#[test]
fn an_editor_sends_a_replica_no_more_than_a_window_ahead_of_the_hosts_answers() {
    let executor = Executor::new(Box::new(|_, _| {
        Serve::Done(Outcome::Err(block_plugin_api::ops::OpError::Unsupported))
    }));
    let mut host = Connection::new(Side::Host);
    let woke = Arc::new(AtomicBool::new(false));
    let waker = crate::Waker::default();
    let woken = Arc::clone(&woke);
    waker.install(move || woken.store(true, Ordering::SeqCst));
    let editor = EditorHost::new(waker);
    let mut link = Link::default();
    link.connect(Uuid::from_u128(1), Uuid::from_u128(2));
    let operations = STREAM_WINDOW as u8 * 2 + 5;
    for operation in 0..operations {
        editor.operate_content(vec![operation]);
    }

    executor.enter(|| link.exchange(INSTANCE, &editor));
    let mut taken = sent(&executor, &mut host);
    assert_eq!(
        taken.len(),
        STREAM_WINDOW as usize,
        "the editor stops at a window of unanswered frames"
    );

    let mut received = Vec::new();
    while !taken.is_empty() {
        woke.store(false, Ordering::SeqCst);
        for (reply, operation) in taken.drain(..) {
            host.complete(reply, Outcome::returned(&()));
            received.push(operation);
        }
        let more = sent(&executor, &mut host);
        assert!(more.is_empty(), "nothing goes until the editor runs again");
        assert!(
            received.len() == operations as usize || woke.load(Ordering::SeqCst),
            "answers that open the window wake the editor"
        );
        executor.enter(|| link.exchange(INSTANCE, &editor));
        taken = sent(&executor, &mut host);
        assert!(taken.len() <= STREAM_WINDOW as usize);
    }
    assert_eq!(
        received,
        (0..operations).collect::<Vec<_>>(),
        "every operation reaches the replica once, in order"
    );
    assert!(editor.take_content_operations().is_empty());
}
