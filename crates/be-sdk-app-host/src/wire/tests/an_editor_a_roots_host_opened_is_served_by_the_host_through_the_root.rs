use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use be_sdk_api::{
    blocks::{BlockSubject, BlockValue},
    guest::blocks::BlockWatch,
    ops::{
        guest::{Executor, Serve, spawn, with_parent},
        is_relayed,
    },
};

use super::*;

const INSTANCE: EditorInstanceId = EditorInstanceId(8);

fn open() -> Opening {
    Opening {
        instance: INSTANCE,
        editor: [1; 16],
        account_id: [4; 16],
        organization_id: [5; 16],
        client_id: [6; 16],
        kind: be_sdk_api::OpeningKind::Block(be_sdk_api::OpenBlock {
            block_id: [2; 16],
            block_type: [3; 16],
            view_block: None,
            editable: true,
            page_origin: String::new(),
            host_palette: false,
        }),
    }
}

/// The root's wire, its host's relayed wire and the app, passing frames until none moves: the
/// app's to the root, which forwards the host's share; the host's to the root, which relays them.
struct Embedded {
    root: Wire,
    host: Wire,
    guest: Executor,
    root_heard: Vec<Heard>,
    host_heard: Vec<Heard>,
}

impl Embedded {
    fn exchange(&mut self) {
        loop {
            let relayed = self.host.take_outbound();
            let moved = !relayed.is_empty();
            self.root
                .relay_forwarded(relayed)
                .expect("the host's frames decode");
            let mut items = Vec::new();
            self.guest
                .step(self.root.take_outbound(), &mut items)
                .expect("the root's frames decode");
            for item in &items {
                self.root
                    .receive(item, &mut self.root_heard)
                    .expect("the app's frames decode");
            }
            for frame in self.root.take_forwarded() {
                self.host
                    .receive(&frame, &mut self.host_heard)
                    .expect("the forwarded frames decode");
            }
            if !moved && items.is_empty() {
                return;
            }
        }
    }
}

#[test]
fn an_editor_a_roots_host_opened_is_served_by_the_host_through_the_root() {
    let editor = Rc::new(Cell::new(None));
    let opened = Rc::clone(&editor);
    let mut embedded = Embedded {
        root: Wire::default(),
        host: Wire::default().relayed(),
        guest: Executor::new(Box::new(move |_, started| {
            opened.set(Some(started.op));
            Serve::Resource {
                instance: Rc::new(()),
                on_drop: Box::new(|| Box::pin(async {})),
            }
        })),
        root_heard: Vec::new(),
        host_heard: Vec::new(),
    };
    embedded.host.open(open());
    embedded.exchange();
    let editor = editor
        .get()
        .expect("the app served the editor the host opened");
    assert!(
        is_relayed(editor),
        "the host's editor is on an op of its own range"
    );

    let told = Rc::new(RefCell::new(Vec::new()));
    let telling = Rc::clone(&told);
    embedded.guest.enter(|| {
        let watch = with_parent(editor, || BlockWatch::new(BlockSubject::History([7; 16])));
        spawn(async move {
            while let Ok(value) = watch.next().await {
                telling.borrow_mut().push(Ok(value));
            }
            telling.borrow_mut().push(Err(()));
        });
    });
    embedded.exchange();
    let watch = match embedded.host_heard.as_slice() {
        [Heard::Blocks(instance, blocks::Call::BlockWatchNew { resource, subject })] => {
            assert_eq!(*instance, INSTANCE);
            assert!(matches!(subject, BlockSubject::History([7, ..])));
            resource.op()
        }
        heard => panic!("the host never heard the watch open: {heard:?}"),
    };
    assert!(
        embedded.root_heard.is_empty(),
        "what the editor asks is the host's, which the root never hears"
    );
    assert_eq!(
        embedded.host.waiting(),
        [watch],
        "the app waits on the host"
    );
    assert!(embedded.root.waiting().is_empty());

    let value = BlockValue::Members(Ok(Vec::new()));
    embedded
        .host
        .answer_watch(watch, be_sdk_api::ops::encode(&value));
    embedded.exchange();
    assert_eq!(
        *told.borrow(),
        [Ok(value)],
        "the host's answer reaches the app through the root"
    );
    assert_eq!(
        embedded.host.waiting(),
        [watch],
        "the app waits on the host again"
    );

    embedded.host_heard.clear();
    embedded.host.close(INSTANCE);
    embedded.exchange();
    assert!(
        embedded
            .host_heard
            .iter()
            .any(|heard| matches!(heard, Heard::Ended(op) if *op == watch)),
        "the host hears the editor's watch end with it: {:?}",
        embedded.host_heard
    );
    assert!(
        embedded.host.waiting().is_empty(),
        "nothing waits on it any more"
    );
    assert_eq!(
        told.borrow().last(),
        Some(&Err(())),
        "the app's wait on the watch ends"
    );
}
