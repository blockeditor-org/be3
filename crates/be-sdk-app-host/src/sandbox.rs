//! What serves an app the sandbox API (`be3:sandbox`) on its wire: the host's
//! `be_wasm_sandbox_host::Server`, which runs an app's workers each on an instance of its own, or
//! an editor test's in-process workers (be-sdk-test). The wire hands it the app's sandbox calls,
//! the ops the app cancels and a poll whenever the host steps the wire, and stops it with the app.

use be_sdk_api::ops::{Connection, OpId, Outcome};
use be_wasm_sandbox_api::embed::sandbox::Call;

pub trait Sandbox {
    /// Takes one of the app's sandbox calls, answering it on `connection` now or later.
    fn call(&mut self, call: Call, connection: &mut Connection);

    /// An op the app canceled; answers whether it was the sandbox's.
    fn canceled(&mut self, op: OpId, connection: &mut Connection) -> bool;

    /// Answers whatever its instances have for the app since.
    fn poll(&mut self, connection: &mut Connection);

    /// Stops every instance, answering each `next` the app waits on with `why`.
    fn stop(self: Box<Self>, why: &str, connection: &mut Connection);
}

/// What a wire serves its app in place of a sandbox it stopped: every later call is answered with
/// why it stopped, so the app hears that, not that its host runs no workers. A compile and an
/// instance's `next` fail with it, an instance starts as one that stopped, and frames sent to one
/// go nowhere.
pub(crate) struct Stopped {
    why: &'static str,
    /// The instances the app made since, each held until the app drops it.
    instances: std::collections::BTreeSet<OpId>,
}

impl Stopped {
    pub(crate) fn new(why: &'static str) -> Self {
        Self {
            why,
            instances: std::collections::BTreeSet::new(),
        }
    }

    pub(crate) fn call(&mut self, call: Call, connection: &mut Connection) {
        match call {
            Call::Compile { reply, .. } => {
                let refused: Result<[u8; 32], String> = Err(self.why.to_owned());
                reply.send(connection, &refused);
            }
            Call::InstanceNew { resource, .. } => {
                self.instances.insert(resource.op());
            }
            Call::InstanceSend { reply, .. } => reply.send(connection, &()),
            Call::InstanceNext { reply, .. } => {
                let stopped: Result<Vec<Vec<u8>>, String> = Err(self.why.to_owned());
                reply.send(connection, &stopped);
            }
        }
    }

    /// The instances the app made since the stop and has not dropped.
    #[cfg(test)]
    pub(crate) fn instances(&self) -> usize {
        self.instances.len()
    }

    /// An instance the app dropped is let go; answers whether the op was one.
    pub(crate) fn canceled(&mut self, op: OpId, connection: &mut Connection) -> bool {
        if !self.instances.remove(&op) {
            return false;
        }
        connection.complete(op, Outcome::returned(&()));
        true
    }
}
