use std::{cell::RefCell, collections::VecDeque, rc::Rc};

use block_plugin_api::{
    guest::blocks::Replica,
    ops::{
        STREAM_WINDOW,
        guest::{Executor, Serve, oneshot, spawn, with_parent},
    },
};

use super::*;

const INSTANCE: EditorInstanceId = EditorInstanceId(4);

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

fn relay(wire: &mut Wire, replica: OpId, source: &mut VecDeque<Vec<u8>>) {
    while let Some(frame) = source.front() {
        if !wire.relay(replica, frame) {
            return;
        }
        source.pop_front();
    }
}

#[test]
fn a_replica_relay_waits_for_the_editors_credit_and_drops_nothing() {
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

    let received = Rc::new(RefCell::new(Vec::new()));
    let (go, reading) = oneshot::<()>();
    let kept = Rc::clone(&received);
    guest.enter(|| {
        let replica = with_parent(editor, || Replica::new([2; 16], [3; 16]));
        let mut frames = replica.frames();
        spawn(async move {
            reading.await;
            while let Some(frame) = frames.recv().await {
                kept.borrow_mut().push(frame);
            }
            drop(replica);
        });
    });
    exchange(&mut wire, &guest, &mut heard);
    let replica = heard
        .iter()
        .find_map(|heard| match heard {
            Heard::Blocks(_, blocks::Call::ReplicaNew { resource, .. }) => Some(resource.op()),
            _ => None,
        })
        .expect("the host heard the replica open");

    let total = STREAM_WINDOW as u8 * 3 + 1;
    let mut source: VecDeque<Vec<u8>> = (0..total).map(|frame| vec![frame]).collect();
    relay(&mut wire, replica, &mut source);
    assert_eq!(
        source.len(),
        total as usize - STREAM_WINDOW as usize,
        "the host stops taking from its source at a window ahead"
    );
    exchange(&mut wire, &guest, &mut heard);
    relay(&mut wire, replica, &mut source);
    assert_eq!(
        source.len(),
        total as usize - STREAM_WINDOW as usize,
        "an editor that takes nothing grants nothing"
    );

    go.send(());
    while !source.is_empty() {
        exchange(&mut wire, &guest, &mut heard);
        let before = source.len();
        relay(&mut wire, replica, &mut source);
        assert!(source.len() < before, "credit lets more through");
    }
    exchange(&mut wire, &guest, &mut heard);
    assert_eq!(
        *received.borrow(),
        (0..total).map(|frame| vec![frame]).collect::<Vec<_>>(),
        "every frame arrives once, in order"
    );
}
