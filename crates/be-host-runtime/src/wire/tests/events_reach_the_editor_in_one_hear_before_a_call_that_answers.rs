use std::{cell::RefCell, rc::Rc};

use be_sdk_api::ops::{
    MethodId, Outcome,
    guest::{Executor, Serve},
};

use super::*;

const FIRST: EditorInstanceId = EditorInstanceId(3);
const SECOND: EditorInstanceId = EditorInstanceId(4);

fn open(instance: EditorInstanceId) -> Opening {
    Opening {
        instance,
        editor: [1; 16],
        account_id: [4; 16],
        organization_id: [5; 16],
        client_id: [6; 16],
        kind: be_sdk_api::OpeningKind::Block(OpenBlock {
            block_id: [2; 16],
            block_type: [3; 16],
            view_block: None,
            editable: true,
            page_origin: String::new(),
            host_palette: false,
        }),
    }
}

fn exchange(wire: &mut Wire, guest: &Executor) {
    loop {
        let mut items = Vec::new();
        guest
            .step(wire.take_outbound(), &mut items)
            .expect("the host's frames decode");
        if items.is_empty() {
            return;
        }
        for item in items {
            wire.receive(&item, &mut Vec::new())
                .expect("the guest's frames decode");
        }
    }
}

fn render(request_id: u64) -> EditorCall {
    EditorCall::RenderClip {
        request_id,
        serial: 1,
        format: "text/plain".to_owned(),
    }
}

#[test]
fn events_reach_the_editor_in_one_hear_before_a_call_that_answers() {
    let calls: Rc<RefCell<Vec<(Option<be_sdk_api::ops::OpId>, MethodId)>>> = Rc::default();
    let seen = Rc::clone(&calls);
    let guest = Executor::new(Box::new(move |_, started| {
        seen.borrow_mut().push((started.parent, started.method));
        match started.parent {
            None => Serve::Resource {
                instance: Rc::new(()),
                on_drop: Box::new(|| Box::pin(async {})),
            },
            Some(_) => Serve::Done(Outcome::returned(&())),
        }
    }));
    let mut wire = Wire::default();
    wire.open(open(FIRST));
    wire.open(open(SECOND));
    exchange(&mut wire, &guest);
    calls.borrow_mut().clear();

    // Alone, each kind of call is one call of its own method.
    wire.call(FIRST, EditorCall::Presence(true));
    wire.flush_hearing();
    exchange(&mut wire, &guest);
    wire.call(FIRST, render(1));
    exchange(&mut wire, &guest);
    let hear = calls.borrow()[0].1;
    let render_clip = calls.borrow()[1].1;
    assert_ne!(hear, render_clip);
    calls.borrow_mut().clear();

    // Events of one editor around another's go as runs, and a call that answers
    // comes after the events queued before it.
    wire.call(FIRST, EditorCall::Presence(true));
    wire.call(FIRST, EditorCall::SetEditable(false));
    wire.call(SECOND, EditorCall::Presence(false));
    wire.call(FIRST, EditorCall::Dismiss);
    wire.call(FIRST, render(2));
    wire.flush_hearing();
    exchange(&mut wire, &guest);

    let methods: Vec<MethodId> = calls.borrow().iter().map(|(_, method)| *method).collect();
    assert_eq!(
        methods,
        [hear, hear, hear, render_clip],
        "two runs of the first editor with the second's between them, then the answer"
    );
}
