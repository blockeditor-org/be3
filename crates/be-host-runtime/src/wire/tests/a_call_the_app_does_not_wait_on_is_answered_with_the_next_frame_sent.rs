use std::{cell::RefCell, rc::Rc};

use be_sdk_api::ops::guest::{Executor, Serve, spawn};

use super::*;

#[test]
fn a_call_the_app_does_not_wait_on_is_answered_with_the_next_frame_sent() {
    let guest = Executor::new(Box::new(|_, _| Serve::Resource {
        instance: Rc::new(()),
        on_drop: Box::new(|| Box::pin(async {})),
    }));
    let mut wire = Wire::default();
    let answered = Rc::new(RefCell::new(None));
    let told = Rc::clone(&answered);
    guest.enter(|| {
        let needed = be_sdk_api::guest::host::frame_needed();
        spawn(async move {
            *told.borrow_mut() = Some(needed.await);
        });
    });
    let mut sent = Vec::new();
    guest
        .step(Vec::new(), &mut sent)
        .expect("the host's frames decode");
    let mut heard = Vec::new();
    for frame in sent {
        wire.receive(&frame, &mut heard)
            .expect("the guest's frames decode");
    }
    assert!(matches!(heard[..], [Heard::FrameNeeded]));
    assert!(
        wire.take_outbound().is_empty(),
        "the answer alone would step the app for nothing, so it waits"
    );

    wire.app_call(AppCall::Configure(1));
    let mut sent = Vec::new();
    guest
        .step(wire.take_outbound(), &mut sent)
        .expect("the host's frames decode");
    assert_eq!(
        *answered.borrow(),
        Some(Ok(())),
        "it goes with the next thing sent to the app"
    );
}
