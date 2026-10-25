use std::{cell::RefCell, rc::Rc};

use block_plugin_api::{
    guest::blocks::Replica,
    ops::{
        OpError,
        guest::{Executor, Serve, spawn, with_parent},
    },
};

use super::*;

const INSTANCE: EditorInstanceId = EditorInstanceId(5);

fn exchange(wire: &mut Wire, guest: &Executor, heard: &mut Vec<Heard>) {
    loop {
        let mut items = Vec::new();
        guest
            .step(wire.take_outbound(), &mut items)
            .expect("the host's frames decode");
        if items.is_empty() {
            return;
        }
        for item in items {
            wire.receive(&item, heard)
                .expect("the guest's frames decode");
        }
    }
}

#[test]
fn a_frame_sent_to_a_replica_is_answered_once_the_host_takes_it_in() {
    let editor = Rc::new(RefCell::new(None));
    let opened = Rc::clone(&editor);
    let guest = Executor::new(Box::new(move |_, started| {
        *opened.borrow_mut() = Some(started.op);
        Serve::Resource {
            instance: Rc::new(()),
            on_drop: Box::new(|| Box::pin(async {})),
        }
    }));
    let mut wire = Wire::default();
    let mut heard = Vec::new();
    wire.open(Opening::Block(block_plugin_api::OpenBlock {
        instance: INSTANCE,
        editor: [1; 16],
        block_id: [2; 16],
        block_type: [3; 16],
        view_block: None,
        account_id: [4; 16],
        organization_id: [5; 16],
        client_id: [6; 16],
        editable: true,
    }));
    exchange(&mut wire, &guest, &mut heard);
    let editor = editor.borrow().expect("the guest served the editor");

    let answers = Rc::new(RefCell::new(Vec::new()));
    let told = Rc::clone(&answers);
    let held = Rc::new(RefCell::new(None));
    let holding = Rc::clone(&held);
    guest.enter(|| {
        let replica = with_parent(editor, || Replica::new([2; 16], [3; 16]));
        let first = replica.send(vec![1]);
        let second = replica.send(vec![2]);
        *holding.borrow_mut() = Some(replica);
        spawn(async move {
            let first = first.await;
            told.borrow_mut().push(first);
            let second = second.await;
            told.borrow_mut().push(second);
        });
    });
    heard.clear();
    exchange(&mut wire, &guest, &mut heard);
    let sends: Vec<OpId> = heard
        .iter()
        .filter_map(|heard| match heard {
            Heard::Blocks(_, blocks::Call::ReplicaSend { reply, .. }) => Some(reply.op()),
            _ => None,
        })
        .collect();
    assert_eq!(sends.len(), 2, "the host hears both frames in order");
    assert!(answers.borrow().is_empty(), "nothing is answered before it is taken in");

    wire.accepted(sends[0]);
    exchange(&mut wire, &guest, &mut heard);
    assert_eq!(*answers.borrow(), [Ok(())], "the first frame was taken in");

    held.borrow_mut().take();
    exchange(&mut wire, &guest, &mut heard);
    wire.accepted(sends[1]);
    exchange(&mut wire, &guest, &mut heard);
    assert_eq!(
        *answers.borrow(),
        [Ok(()), Err(OpError::Canceled)],
        "a frame still waiting when its replica closes ends canceled, once"
    );
}
