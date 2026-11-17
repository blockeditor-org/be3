use std::{
    cell::RefCell,
    collections::{BTreeMap, HashMap},
    rc::Rc,
};

use be_sdk_api::ops::{Connection, OpError, OpId, Outcome};
use be_sdk_beui::worker::{Hosted, Work};
use be_wasm_sandbox_api::{embed::sandbox::Call, sandbox::Source};

type Start = fn(&mut Vec<Vec<u8>>) -> Result<Hosted, String>;

type Frames = Result<Vec<Vec<u8>>, String>;

/// A `next` the editor waits on: its op, and how to answer it.
type Next = (OpId, Box<dyn FnOnce(&Frames) -> (OpId, Outcome)>);

/// Why a test's editor may not compile a module, as no app may.
const NO_COMPILES: &str = "an app may not compile a module from bytes";

/// The workers an editor spawned (be_sdk::worker), served as the host serves an app's, but each
/// in the test's own instance (`Hosted`), so a job is done by the time the editor next steps.
/// A test names the workers it serves with `BeuiTest::serve_worker`. Its bookkeeping (an instance
/// per resource, frames held until a `next` asks, a stop answering every `next` after) follows
/// `be_wasm_sandbox_host::Server`'s, which serves the same calls for real: change both together.
#[derive(Default)]
pub(crate) struct Workers {
    served: HashMap<String, Start>,
    running: BTreeMap<OpId, Running>,
}

struct Running {
    /// The worker, or why it could not start or stopped.
    work: Result<Hosted, String>,
    sent: Vec<Vec<u8>>,
    next: Option<Next>,
}

impl Workers {
    pub(crate) fn serve<W: Work>(&mut self, module: &str) {
        self.served.insert(module.to_owned(), Hosted::start::<W>);
    }

    /// Takes one of the editor's sandbox calls.
    fn call(&mut self, call: Call, connection: &mut Connection) {
        match call {
            Call::Compile { reply, .. } => {
                let refused: Result<[u8; 32], String> = Err(NO_COMPILES.to_owned());
                reply.send(connection, &refused);
            }
            Call::InstanceNew {
                resource, source, ..
            } => {
                let mut sent = Vec::new();
                let work = match source {
                    Source::App(staged) if staged.app.is_empty() => {
                        match self.served.get(&staged.module) {
                            Some(start) => start(&mut sent),
                            None => Err(format!(
                                "the test serves no worker {}: serve it with BeuiTest::serve_worker",
                                staged.module
                            )),
                        }
                    }
                    _ => Err("an app may spawn only its own workers".to_owned()),
                };
                self.running.insert(
                    resource.op(),
                    Running {
                        work,
                        sent,
                        next: None,
                    },
                );
            }
            Call::InstanceSend {
                this,
                reply,
                frames,
            } => {
                if let Some(running) = self.running.get_mut(&this.op())
                    && let Ok(work) = &mut running.work
                    && let Err(failure) = work.step(frames, &mut running.sent)
                {
                    running.work = Err(failure);
                }
                reply.send(connection, &());
            }
            Call::InstanceNext { this, reply } => {
                match self.running.get_mut(&this.op()) {
                    Some(running) => {
                        running.next =
                            Some((reply.op(), Box::new(move |value| reply.answer(value))));
                    }
                    None => reply.fail(connection, OpError::Canceled),
                }
            }
        }
    }

    /// Answers each `next` the editor waits on that has something to answer.
    fn answer(&mut self, connection: &mut Connection) {
        for running in self.running.values_mut() {
            let answer = match &running.work {
                _ if running.next.is_none() => continue,
                _ if !running.sent.is_empty() => Ok(std::mem::take(&mut running.sent)),
                Err(failure) => Err(failure.clone()),
                Ok(_) => continue,
            };
            if let Some((_, next)) = running.next.take() {
                let (op, outcome) = next(&answer);
                connection.complete(op, outcome);
            }
        }
    }

    /// An op the editor canceled: a worker it dropped stops, and a `next` it no longer waits for
    /// is answered as canceled. Answers whether the op was a worker's.
    fn canceled(&mut self, op: OpId, connection: &mut Connection) -> bool {
        if let Some(running) = self.running.remove(&op) {
            if let Some((next, _)) = running.next {
                connection.complete(next, Outcome::Err(OpError::Canceled));
            }
            connection.complete(op, Outcome::returned(&()));
            return true;
        }
        for running in self.running.values_mut() {
            if running.next.as_ref().is_some_and(|(next, _)| *next == op) {
                running.next = None;
                connection.complete(op, Outcome::Err(OpError::Canceled));
                return true;
            }
        }
        false
    }
}

/// The test's workers as the editor's wire serves them (`Sandbox`), while the test still names
/// the workers it serves (`BeuiTest::serve_worker`).
#[derive(Clone, Default)]
pub(crate) struct Shared(pub(crate) Rc<RefCell<Workers>>);

impl be_sdk_app_host::sandbox::Sandbox for Shared {
    fn call(&mut self, call: Call, connection: &mut Connection) {
        self.0.borrow_mut().call(call, connection);
    }

    fn canceled(&mut self, op: OpId, connection: &mut Connection) -> bool {
        self.0.borrow_mut().canceled(op, connection)
    }

    fn poll(&mut self, connection: &mut Connection) {
        self.0.borrow_mut().answer(connection);
    }

    fn stop(self: Box<Self>, why: &str, connection: &mut Connection) {
        let mut workers = self.0.borrow_mut();
        for running in std::mem::take(&mut workers.running).into_values() {
            if let Some((_, next)) = running.next {
                let (op, outcome) = next(&Err(why.to_owned()));
                connection.complete(op, outcome);
            }
        }
    }
}
