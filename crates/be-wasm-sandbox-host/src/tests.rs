use std::{
    cell::RefCell,
    future::Future,
    rc::Rc,
    sync::{Arc, mpsc},
    time::Duration,
};

use be_wasm_host::{Stored, library_file_name};
use be_wasm_ops::{Incoming, Side};
use be_wasm_sandbox_api::{
    embed, guest,
    sandbox::{Config, Source, StagedModule},
};

use super::*;

mod a_guest_compiles_a_module_and_talks_to_an_instance_of_it;
mod a_guest_spawns_a_staged_app_by_its_id_and_talks_to_it;
mod an_app_spawns_its_own_worker_and_exchanges_messages_with_it;
mod an_app_spawns_neither_another_apps_module_nor_a_module_from_bytes;
mod an_instance_of_an_app_no_staged_app_has_the_id_of_says_so;
mod an_instance_that_stopped_says_why;
mod an_instance_the_guest_drops_stops;

const ECHO: &[u8] = include_bytes!(env!("GUEST_ECHO_WASM"));

/// The echo guest as a staged app, linked against the base library.
const ECHO_APP: &[u8] = include_bytes!(env!("GUEST_ECHO_APP_WASM"));

const BASE_LIBRARY: &[u8] = include_bytes!(env!("BASE_LIBRARY_WASM"));

/// The echo app's worker (crates/be-sdk-worker-echo), linked against the base library.
const ECHO_WORKER: &[u8] = include_bytes!(env!("ECHO_WORKER_WASM"));

const ECHO_ENTRY: &str = "echo.wasm";

/// Where the echo app's worker is staged, as be-tool-buck stages it.
const ECHO_WORKER_PATH: &str = "workers/be3.echo/echo-worker.wasm";

/// Finds the one staged app, the echo app, with its module, its worker and the base library staged
/// as be-tool-buck stages them: the library named by its identity.
fn staged(app_id: &str, module: &str) -> Result<Staged, String> {
    if app_id != be_wasm_guest_echo::APP_ID {
        return Err(format!("no staged app has the id {app_id}"));
    }
    let entry = match module {
        ECHO_ENTRY => ECHO_ENTRY,
        be_sdk_worker_echo::MODULE => ECHO_WORKER_PATH,
        _ => return Err(format!("{app_id} stages no module named {module:?}")),
    };
    let library = library_file_name(&be_wasm_dylink::identity(BASE_LIBRARY));
    Ok(Staged::new(
        entry.to_owned(),
        Arc::new(move |name| match name {
            ECHO_ENTRY => Ok(Stored::Bytes(ECHO_APP.to_vec())),
            ECHO_WORKER_PATH => Ok(Stored::Bytes(ECHO_WORKER.to_vec())),
            _ if name == library => Ok(Stored::Bytes(BASE_LIBRARY.to_vec())),
            _ => Err(format!("{name} is not staged")),
        }),
    ))
}

/// The echo app's own module, its entry point.
fn echo_app() -> Source {
    Source::App(StagedModule {
        app: be_wasm_guest_echo::APP_ID.to_owned(),
        module: ECHO_ENTRY.to_owned(),
    })
}

fn config() -> Config {
    Config {
        name: "echo".to_owned(),
        memory_limit: 16 << 20,
    }
}

/// An app links its libraries into one shared memory, which the base library alone fills more of
/// than a module from bytes needs.
fn app_config() -> Config {
    Config {
        name: "echo app".to_owned(),
        memory_limit: 1 << 30,
    }
}

/// A guest run in process against a `Server`, as a host serves one.
struct Pump {
    executor: be_wasm_ops::guest::Executor,
    host: Connection,
    server: Server,
    woken: mpsc::Receiver<()>,
    to_guest: Vec<Vec<u8>>,
}

impl Pump {
    /// A guest served as the root guest is, which may spawn any staged app's modules.
    fn new() -> Self {
        Self::serving(|wake| Server::new(wake, Box::new(staged)))
    }

    /// A guest served as the echo app is: it may spawn only the echo app's modules.
    fn echo_app() -> Self {
        be_sdk_worker::install_app(be_wasm_guest_echo::APP_ID);
        Self::serving(|wake| {
            Server::for_app(wake, be_wasm_guest_echo::APP_ID.to_owned(), Box::new(staged))
        })
    }

    fn serving(server: impl FnOnce(Wake) -> Server) -> Self {
        let (wake, woken) = mpsc::channel();
        Self {
            executor: guest::executor(()),
            host: Connection::new(Side::Host),
            server: server(Arc::new(move || {
                let _ = wake.send(());
            })),
            woken,
            to_guest: Vec::new(),
        }
    }

    /// Runs `task` in the guest, passing frames both ways, until it finishes.
    fn run<T: 'static>(&mut self, task: impl Future<Output = T> + 'static) -> T {
        let done = Rc::new(RefCell::new(None));
        let finished = Rc::clone(&done);
        self.executor.enter(|| {
            be_wasm_ops::guest::spawn(async move {
                *finished.borrow_mut() = Some(task.await);
            });
        });
        loop {
            let mut from_guest = Vec::new();
            self.executor
                .step(std::mem::take(&mut self.to_guest), &mut from_guest)
                .unwrap();
            for frame in &from_guest {
                self.host.receive(frame).unwrap();
            }
            while let Some(event) = self.host.next_event() {
                match event {
                    Incoming::Started(started) => {
                        if let Some(call) = embed::sandbox::Call::accept(&mut self.host, started) {
                            self.server.call(call, &mut self.host);
                        }
                    }
                    Incoming::Canceled { op } => {
                        self.server.canceled(op, &mut self.host);
                    }
                    _ => {}
                }
            }
            self.server.poll(&mut self.host);
            self.to_guest = self.host.take_outbound();
            if let Some(value) = done.borrow_mut().take() {
                return value;
            }
            if self.to_guest.is_empty() && from_guest.is_empty() {
                self.woken
                    .recv_timeout(Duration::from_secs(60))
                    .expect("the sandbox answers in time");
            }
        }
    }
}
