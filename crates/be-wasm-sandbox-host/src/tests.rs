use std::{
    cell::RefCell,
    future::Future,
    rc::Rc,
    sync::{Arc, mpsc},
    time::Duration,
};

use be_wasm_ops::{Incoming, Side};
use be_wasm_sandbox_api::{embed, guest, sandbox::Config};

use super::*;

mod a_guest_compiles_a_module_and_talks_to_an_instance_of_it;
mod an_instance_that_stopped_says_why;
mod an_instance_the_guest_drops_stops;

const ECHO: &[u8] = include_bytes!(env!("GUEST_ECHO_WASM"));

fn config() -> Config {
    Config {
        name: "echo".to_owned(),
        memory_limit: 16 << 20,
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
    fn new() -> Self {
        let (wake, woken) = mpsc::channel();
        Self {
            executor: guest::executor(()),
            host: Connection::new(Side::Host),
            server: Server::new(Arc::new(move || {
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
