use std::{cell::RefCell, rc::Rc, sync::Arc};

use be_wasm_sandbox_api::guest::sandbox;

use super::a_watch_of_an_extension_the_manifest_does_not_grant_is_denied::{exchange, opened};
use super::an_app_may_start_only_its_own_apps_modules::{APP, Served, spawned, unstaged};
use super::*;

/// Once the wire stopped its app's sandbox, every sandbox call the app makes later is answered
/// with why it stopped, not refused as if the host ran no workers.
#[test]
fn a_stopped_sandbox_answers_later_calls_with_why() {
    let mut wire = Wire::default().with_sandbox(Served(be_wasm_sandbox_host::Server::for_app(
        Arc::new(|| {}),
        APP.to_owned(),
        Box::new(unstaged),
    )));
    let (guest, _, mut heard) = opened(&mut wire);

    wire.stop_sandbox();
    assert!(!wire.serves_sandbox(), "a stopped sandbox runs no workers");

    let next = spawned(&guest, "");
    let compiled = Rc::new(RefCell::new(None));
    let told = Rc::clone(&compiled);
    guest.enter(|| {
        let compiling = sandbox::compile(b"\0asm\x01\0\0\0".to_vec());
        be_sdk_api::ops::guest::spawn(async move {
            *told.borrow_mut() = Some(compiling.await.expect("the host answers"));
        });
    });
    exchange(&mut wire, &guest, &mut heard);

    assert_eq!(
        next.borrow_mut().take(),
        Some(Err(WORKERS_STOPPED.to_owned())),
        "a worker started after the stop hears why it will not run"
    );
    assert_eq!(
        compiled.borrow_mut().take(),
        Some(Err(WORKERS_STOPPED.to_owned())),
        "a compile after the stop hears why too"
    );
}
