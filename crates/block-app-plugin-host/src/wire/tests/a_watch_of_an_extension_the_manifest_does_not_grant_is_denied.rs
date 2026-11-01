use std::{cell::RefCell, rc::Rc};

use block_plugin_api::{
    AdministratorWatch, PowerAction, PowerWatch, Watched,
    guest::desktop::power,
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

type Answer = Rc<RefCell<Option<Result<(), OpError>>>>;

fn watched<W: Watched>(guest: &Executor, editor: OpId) -> Answer {
    let answer = Answer::default();
    let told = Rc::clone(&answer);
    guest.enter(|| {
        let watch = with_parent(editor, W::open);
        spawn(async move {
            *told.borrow_mut() = Some(watch.next().await.map(drop));
        });
    });
    answer
}

fn opened(wire: &mut Wire) -> (Executor, OpId, Vec<Heard>) {
    let editor = Rc::new(std::cell::Cell::new(None));
    let served = Rc::clone(&editor);
    let guest = Executor::new(Box::new(move |_, started| {
        served.set(Some(started.op));
        Serve::Resource {
            instance: Rc::new(()),
            on_drop: Box::new(|| Box::pin(async {})),
        }
    }));
    let mut heard = Vec::new();
    wire.open(open());
    exchange(wire, &guest, &mut heard);
    let editor = editor.get().expect("the guest served the editor");
    (guest, editor, heard)
}

#[test]
fn a_watch_of_an_extension_the_manifest_does_not_grant_is_denied() {
    let mut wire = Wire::default();
    let (guest, editor, mut heard) = opened(&mut wire);
    let power = watched::<PowerWatch>(&guest, editor);
    let administrator = watched::<AdministratorWatch>(&guest, editor);
    guest.enter(|| with_parent(editor, || power::request(PowerAction::PowerOff)).detach());
    exchange(&mut wire, &guest, &mut heard);
    assert!(
        matches!(*power.borrow(), Some(Err(_))),
        "a plugin that names no extension may not watch one"
    );
    assert!(
        matches!(heard.as_slice(), [Heard::Watch(INSTANCE, open)] if *open == AdministratorWatch::OPEN),
        "the account's watches are core, and the call is dropped: {heard:?}"
    );
    assert_eq!(*administrator.borrow(), None, "it waits for the value");

    let mut wire = Wire::granting(vec!["be3:desktop".to_owned()]);
    let (guest, editor, mut heard) = opened(&mut wire);
    let power = watched::<PowerWatch>(&guest, editor);
    guest.enter(|| with_parent(editor, || power::request(PowerAction::PowerOff)).detach());
    exchange(&mut wire, &guest, &mut heard);
    match cfg!(target_os = "linux") {
        true => {
            assert_eq!(*power.borrow(), None, "a granted watch waits for the value");
            assert!(matches!(
                heard.as_slice(),
                [Heard::Watch(INSTANCE, open), Heard::Published(INSTANCE, _)]
                    if *open == PowerWatch::OPEN
            ));
        }
        false => assert!(
            matches!(*power.borrow(), Some(Err(_))),
            "a host with no Linux session has none of it"
        ),
    }
}
