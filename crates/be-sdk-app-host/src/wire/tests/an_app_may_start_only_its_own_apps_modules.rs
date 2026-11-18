use std::{cell::RefCell, rc::Rc, sync::Arc};

use be_wasm_sandbox_api::{
    guest::sandbox::{self, Instance},
    sandbox::{Config, Source, StagedModule},
};

use super::a_watch_of_an_extension_the_manifest_does_not_grant_is_denied::{exchange, opened};
use super::*;

const APP: &str = "be3.echo";

/// The app's own modules are asked of the host's loader, and none is staged in this test.
fn unstaged(app: &str, module: &str) -> Result<be_wasm_host::sandbox::Staged, String> {
    Err(format!("asked for {app}'s {module}"))
}

/// The host's sandbox, as be-host-runtime serves it (`AppSandbox`).
struct Served(be_wasm_sandbox_host::Server);

impl crate::sandbox::Sandbox for Served {
    fn call(
        &mut self,
        call: be_wasm_sandbox_api::embed::sandbox::Call,
        connection: &mut Connection,
    ) {
        self.0.call(call, connection);
    }

    fn canceled(&mut self, op: OpId, connection: &mut Connection) -> bool {
        self.0.canceled(op, connection)
    }

    fn poll(&mut self, connection: &mut Connection) {
        self.0.poll(connection);
    }

    fn stop(self: Box<Self>, why: &str, connection: &mut Connection) {
        self.0.stop(why, connection);
    }
}

fn config() -> Config {
    Config {
        name: "worker".to_owned(),
        memory_limit: 1 << 20,
    }
}

/// What the instance's first `next` answered.
type Next = Rc<RefCell<Option<Result<Vec<Vec<u8>>, String>>>>;

fn spawned(guest: &be_sdk_api::ops::guest::Executor, app: &str) -> Next {
    let next = Next::default();
    let told = Rc::clone(&next);
    let source = Source::App(StagedModule {
        app: app.to_owned(),
        module: "worker.wasm".to_owned(),
    });
    guest.enter(|| {
        let instance = Instance::new(source, config());
        be_sdk_api::ops::guest::spawn(async move {
            let answered = instance.next().await.expect("the host answers");
            *told.borrow_mut() = Some(answered);
        });
    });
    next
}

/// The host serves each app the sandbox with a loader asked only for its own app's modules, and
/// refuses it a module from bytes, which stay the root guest's.
#[test]
fn an_app_may_start_only_its_own_apps_modules() {
    let mut wire = Wire::default().with_sandbox(Served(be_wasm_sandbox_host::Server::for_app(
        Arc::new(|| {}),
        APP.to_owned(),
        Box::new(unstaged),
    )));
    let (guest, _, mut heard) = opened(&mut wire);

    let own = spawned(&guest, "");
    let other = spawned(&guest, "be3.notes");
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
        own.borrow_mut().take(),
        Some(Err("asked for be3.echo's worker.wasm".to_owned())),
        "an empty id names the app's own, which is asked of its loader"
    );
    assert_eq!(
        other.borrow_mut().take(),
        Some(Err(
            "be3.echo may spawn only its own modules, not be3.notes's".to_owned()
        )),
        "another app's module is refused before the loader hears of it"
    );
    assert_eq!(
        compiled.borrow_mut().take(),
        Some(Err(be_wasm_sandbox_host::APPS_COMPILE_NOTHING.to_owned()))
    );
}
