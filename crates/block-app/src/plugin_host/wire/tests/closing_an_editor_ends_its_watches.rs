use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use block_plugin_api::{
    guest::blocks::HistoryWatch,
    ops::{
        OpError,
        guest::{Executor, Serve, spawn, with_parent},
    },
};

use super::*;

const INSTANCE: EditorInstanceId = EditorInstanceId(3);

fn open() -> Opening {
    Opening::Block(block_plugin_api::OpenBlock {
        instance: INSTANCE,
        editor: [1; 16],
        block_id: [2; 16],
        block_type: [3; 16],
        view_block: None,
        account_id: [4; 16],
        organization_id: [5; 16],
        client_id: [6; 16],
        editable: true,
    })
}

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
fn closing_an_editor_ends_its_watches() {
    let editor = Rc::new(Cell::new(None));
    let opened = Rc::clone(&editor);
    let guest = Executor::new(Box::new(move |_, started| {
        opened.set(Some(started.op));
        Serve::Resource {
            instance: Rc::new(()),
            on_drop: Box::new(|| Box::pin(async {})),
        }
    }));
    let mut wire = Wire::default();
    let mut heard = Vec::new();
    wire.open(open());
    exchange(&mut wire, &guest, &mut heard);
    let editor = editor.get().expect("the guest served the editor");

    let ended = Rc::new(RefCell::new(None));
    let told = Rc::clone(&ended);
    guest.enter(|| {
        let watch = with_parent(editor, || HistoryWatch::new([7; 16]));
        spawn(async move {
            let answer = watch.next().await;
            *told.borrow_mut() = Some(answer.map(drop));
        });
    });
    exchange(&mut wire, &guest, &mut heard);
    let watch = match heard.as_slice() {
        [Heard::Blocks(instance, blocks::Call::HistoryWatchNew { resource, block })] => {
            assert_eq!((*instance, *block), (INSTANCE, [7; 16]));
            resource.op()
        }
        _ => panic!("the host never heard the watch open"),
    };
    assert_eq!(wire.waiting(), [watch], "the guest waits on the watch");
    heard.clear();

    wire.close(INSTANCE);
    exchange(&mut wire, &guest, &mut heard);

    assert!(
        heard
            .iter()
            .any(|heard| matches!(heard, Heard::Ended(op) if *op == watch)),
        "the host hears the editor's watch end with it"
    );
    assert!(wire.waiting().is_empty(), "nothing waits on it any more");
    assert_eq!(
        *ended.borrow(),
        Some(Err(OpError::Canceled)),
        "the guest's wait on the watch ends"
    );
}
