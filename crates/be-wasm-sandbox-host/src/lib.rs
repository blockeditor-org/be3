//! The host's half of the sandbox API for modules from bytes (`be3:sandbox`,
//! crates/be-wasm-sandbox-api), natively: one `Server` per guest that is granted it, answering its
//! calls from `be_wasm_host::sandbox` (plans/host_as_platform_adapter.md, The sandbox API). It
//! will serve the root guest (step 4); only its tests use it today.

use std::{
    collections::{BTreeMap, HashMap},
    sync::Arc,
};

use be_wasm_guest_host::Wake;
use be_wasm_host::sandbox::{self, Compiling, Instance, Module, ModuleId};
use be_wasm_ops::{Connection, OpError, OpId, Outcome};
use be_wasm_sandbox_api::embed::sandbox::Call;

/// Answers a value once it is known: a reply held until then.
type Answer<T> = Box<dyn FnOnce(&T) -> (OpId, Outcome)>;

type Frames = Result<Vec<Vec<u8>>, String>;

/// Serves one guest the sandbox. Every module it compiled stays compiled, and every instance it
/// spawned runs, while the server is held.
pub struct Server {
    wake: Wake,
    modules: HashMap<ModuleId, Module>,
    compiling: Vec<(Compiling, Answer<Result<ModuleId, String>>)>,
    instances: BTreeMap<OpId, Served>,
}

struct Served {
    /// The instance, or why it could not start or stopped.
    instance: Result<Instance, String>,
    received: Vec<Vec<u8>>,
    next: Option<(OpId, Answer<Frames>)>,
}

impl Server {
    /// `wake` is called whenever a compile finishes or an instance sent frames or stopped, after
    /// which the guest's host calls `poll`.
    pub fn new(wake: Wake) -> Self {
        Self {
            wake,
            modules: HashMap::new(),
            compiling: Vec::new(),
            instances: BTreeMap::new(),
        }
    }

    /// Takes one of the guest's calls.
    pub fn call(&mut self, call: Call, connection: &mut Connection) {
        match call {
            Call::Compile { reply, bytes } => {
                let compiling = sandbox::compile(bytes, Arc::clone(&self.wake));
                self.compiling
                    .push((compiling, Box::new(move |value| reply.answer(value))));
            }
            Call::InstanceNew {
                resource,
                module,
                config,
            } => {
                let instance = match self.modules.get(&module) {
                    Some(module) => sandbox::spawn(
                        module,
                        sandbox::Config {
                            name: config.name,
                            memory_limit: usize::try_from(config.memory_limit)
                                .unwrap_or(usize::MAX),
                        },
                        Arc::clone(&self.wake),
                    ),
                    None => Err("this guest compiled no module with that id".to_owned()),
                };
                self.instances.insert(
                    resource.op(),
                    Served {
                        instance,
                        received: Vec::new(),
                        next: None,
                    },
                );
            }
            Call::InstanceSend {
                this,
                reply,
                frames,
            } => {
                if let Some(served) = self.instances.get_mut(&this.op())
                    && let Ok(instance) = &mut served.instance
                    && let Err(failure) = instance.send(frames)
                {
                    served.instance = Err(failure);
                }
                reply.send(connection, &());
            }
            Call::InstanceNext { this, reply } => match self.instances.get_mut(&this.op()) {
                Some(served) => {
                    let op = reply.op();
                    served.next = Some((op, Box::new(move |value| reply.answer(value))));
                }
                None => reply.fail(connection, OpError::Canceled),
            },
        }
        self.poll(connection);
    }

    /// An op the guest canceled: an instance it dropped stops, and a `next` it no longer waits for
    /// is answered as canceled. Answers whether the op was the sandbox's.
    pub fn canceled(&mut self, op: OpId, connection: &mut Connection) -> bool {
        if let Some(served) = self.instances.remove(&op) {
            if let Some((next, _)) = served.next {
                connection.complete(next, Outcome::Err(OpError::Canceled));
            }
            connection.complete(op, Outcome::returned(&()));
            return true;
        }
        for served in self.instances.values_mut() {
            if served
                .next
                .as_ref()
                .is_some_and(|(waiting, _)| *waiting == op)
            {
                served.next = None;
                connection.complete(op, Outcome::Err(OpError::Canceled));
                return true;
            }
        }
        false
    }

    /// Answers whatever finished since: compiles, and the frames instances sent or why they
    /// stopped.
    pub fn poll(&mut self, connection: &mut Connection) {
        let mut compiling = Vec::with_capacity(self.compiling.len());
        for (compile, answer) in std::mem::take(&mut self.compiling) {
            let Some(done) = compile.done() else {
                compiling.push((compile, answer));
                continue;
            };
            let id = done.map(|module| {
                let id = module.id();
                self.modules.insert(id, module);
                id
            });
            let (op, outcome) = answer(&id);
            connection.complete(op, outcome);
        }
        self.compiling = compiling;
        for served in self.instances.values_mut() {
            if let Ok(instance) = &mut served.instance {
                match instance.receive() {
                    Ok(frames) => served.received.extend(frames),
                    Err(failure) => served.instance = Err(failure),
                }
            }
            let answer = match &served.instance {
                _ if served.next.is_none() => continue,
                _ if !served.received.is_empty() => Ok(std::mem::take(&mut served.received)),
                Err(failure) => Err(failure.clone()),
                Ok(_) => continue,
            };
            if let Some((_, next)) = served.next.take() {
                let (op, outcome) = next(&answer);
                connection.complete(op, outcome);
            }
        }
    }
}

#[cfg(test)]
mod tests;
