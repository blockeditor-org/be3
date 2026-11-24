use std::{
    cell::RefCell,
    rc::Rc,
    sync::{Mutex, mpsc},
    time::Duration,
};

use be_sdk_api::ops::guest::{Executor, Serve};
use be_sdk_worker::Worker;
use be_sdk_worker_echo::{ECHOED, MODULE, STARTED};

use super::*;

/// A module buck built (crates/be-host-runtime/BUCK names it).
fn module(variable: &str) -> Vec<u8> {
    let path = std::env::var(variable).unwrap_or_else(|_| {
        panic!("{variable} names a module buck built: run this test with ./scripts/test")
    });
    std::fs::read(&path).unwrap_or_else(|error| panic!("{path} could not be read: {error}"))
}

const MANIFEST: &str = r#"{"id": "be3.echo", "name": "Echo", "version": "0.1.0",
    "entry_point": "echo.wasm", "workers": ["echo-worker.wasm"],
    "editors": {"echo": {"id": "00000000-0000-4000-8000-00000000ec40",
        "display_name": "Echo", "icon": "x", "views": ["Main"]}}}"#;

/// Stages the echo app and its worker as be-tool-buck stages them, in a directory of their own
/// that discovery reads the staged files from (this test runs in a process of its own).
fn stage() -> std::path::PathBuf {
    let bundle = std::env::temp_dir().join(format!("be3-worker-bundle-{}", std::process::id()));
    let workers = bundle.join("workers/be3.echo");
    std::fs::create_dir_all(&workers).unwrap();
    let library = module("BASE_LIBRARY_WASM");
    let library_name = be_wasm_host::library_file_name(&be_wasm_dylink::identity(&library));
    std::fs::write(bundle.join(library_name), library).unwrap();
    std::fs::write(bundle.join("echo.wasm"), module("ECHO_APP_WASM")).unwrap();
    std::fs::write(workers.join(MODULE), module("ECHO_WORKER_WASM")).unwrap();
    let staged = bundle.clone();
    be_host_stack::discovery::use_assets(move |path| {
        std::fs::read(staged.join(path)).map_err(|error| format!("{path}: {error}"))
    });
    bundle
}

/// The app's side of the wire: an executor that serves nothing, which the worker is spawned on.
fn guest() -> Executor {
    Executor::new(Box::new(|_, _| Serve::Resource {
        instance: Rc::new(()),
        on_drop: Box::new(|| Box::pin(async {})),
    }))
}

/// The app's worker is found among the staged files by the host's own loader, started through the
/// wire the host serves the app, answers it, and stops with the app when the app fails.
#[test]
fn an_apps_worker_answers_through_the_hosts_wire_and_stops_when_the_app_fails() {
    let bundle = stage();
    let _installed =
        be_host_stack::discovery::install_for_testing(&[("be3.echo.app.json", MANIFEST)]);
    let (wake, woken) = mpsc::channel();
    let wake = Mutex::new(wake);
    be_host_base::host::install_waker(move || {
        let _ = wake
            .lock()
            .unwrap_or_else(|held| held.into_inner())
            .send(());
    });
    let app = be_sdk_manifest::manifest_from_json(MANIFEST).unwrap();
    let mut backend = Backend::new(&app);
    assert!(
        backend.wire_mut().serves_sandbox(),
        "the host serves every app a sandbox"
    );

    let guest = guest();
    let received = Rc::new(RefCell::new(Vec::new()));
    let told = Rc::clone(&received);
    guest.enter(|| {
        let mut worker = Worker::<Vec<u8>, Vec<u8>>::spawn(MODULE);
        worker.send(&b"hello".to_vec());
        be_sdk_api::ops::guest::spawn(async move {
            for _ in 0..3 {
                let message = worker.receive().await;
                told.borrow_mut().push(message);
            }
        });
    });
    let mut heard = Vec::new();
    while received.borrow().len() < 2 {
        let wire = backend.wire_mut();
        let mut from_guest = Vec::new();
        guest
            .step(wire.take_outbound(), &mut from_guest)
            .expect("the host's frames decode");
        for frame in &from_guest {
            wire.receive(frame, &mut heard)
                .expect("the app's frames decode");
        }
        wire.poll_sandbox();
        if from_guest.is_empty() && !wire.has_outbound() && received.borrow().len() < 2 {
            woken
                .recv_timeout(Duration::from_secs(60))
                .expect("the worker answers in time");
        }
    }
    assert_eq!(
        *received.borrow(),
        vec![Ok(STARTED.to_vec()), Ok([ECHOED, b"hello"].concat())]
    );

    backend.fail("the app trapped".to_owned());
    assert!(
        !backend.wire_mut().serves_sandbox(),
        "an app that fails takes its workers with it"
    );
    let mut from_guest = Vec::new();
    guest
        .step(backend.wire_mut().take_outbound(), &mut from_guest)
        .expect("the host's frames decode");
    assert_eq!(
        received.borrow().get(2),
        Some(&Err(crate::wire::WORKERS_STOPPED.to_owned())),
        "the worker's app hears that it stopped"
    );
    assert_eq!(backend.take_error().as_deref(), Some("the app trapped"));
    let _ = std::fs::remove_dir_all(bundle);
}
