use std::{cell::RefCell, rc::Rc, sync::Arc};

use be_wasm_sandbox_api::{
    guest::sandbox::Instance,
    sandbox::{Config, Source, StagedModule},
};

use super::a_watch_of_an_extension_the_manifest_does_not_grant_is_denied::{exchange, opened};
use super::an_app_may_start_only_its_own_apps_modules::{APP, Served, unstaged};
use super::*;

/// An instance the app makes after its sandbox stopped takes what it is sent, answering the
/// send, and is let go when the app drops it, so nothing it made is held.
#[test]
fn an_instance_made_after_a_sandbox_stopped_is_sent_to_and_dropped() {
    let mut wire = Wire::default().with_sandbox(Served(be_wasm_sandbox_host::Server::for_app(
        Arc::new(|| {}),
        APP.to_owned(),
        Box::new(unstaged),
    )));
    let (guest, _, mut heard) = opened(&mut wire);
    wire.stop_sandbox();

    let sent = Rc::new(RefCell::new(None));
    let told = Rc::clone(&sent);
    guest.enter(|| {
        let instance = Instance::new(
            Source::App(StagedModule {
                app: String::new(),
                module: "worker.wasm".to_owned(),
            }),
            Config {
                name: "worker".to_owned(),
                memory_limit: 1 << 20,
            },
        );
        be_sdk_api::ops::guest::spawn(async move {
            *told.borrow_mut() = Some(instance.send(vec![b"unheard".to_vec()]).await);
            drop(instance);
        });
    });
    exchange(&mut wire, &guest, &mut heard);

    assert_eq!(
        sent.borrow_mut().take(),
        Some(Ok(())),
        "a send to the instance is answered"
    );
    assert_eq!(
        wire.stopped.as_ref().map(Stopped::instances),
        Some(0),
        "the dropped instance is let go"
    );
}
