use block_guest_abi::Flow;
use block_plugin_api::ops::guest::Executor;

use crate::wire::Shared;
use crate::{Waker, runtime::Runtime};

pub struct Editor {
    runtime: Runtime,
    wire: Shared,
    executor: Executor,
}

pub(crate) struct Started {
    pub(crate) apps: Vec<(uuid::Uuid, crate::screens::Opener)>,
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) version: String,
}

impl Editor {
    pub(crate) fn start(
        started: Started,
        waker: block_guest_abi::Waker,
        outbound: &mut Vec<Vec<u8>>,
    ) -> Result<Self, String> {
        let Started {
            apps,
            id,
            name,
            version,
        } = started;
        let runtime = Runtime::new(apps, &id, &name, &version, Waker::new(move || waker.wake()));
        let wire = Shared::default();
        let executor = wire.executor();
        executor.enter(|| {
            wire.connect(runtime.hello());
            wire.watch();
            wire.watch_shell();
        });
        executor.step(Vec::new(), outbound)?;
        Ok(Self {
            runtime,
            wire,
            executor,
        })
    }

    pub fn step(
        &mut self,
        inbound: Vec<Vec<u8>>,
        woken: bool,
        outbound: &mut Vec<Vec<u8>>,
    ) -> Result<Flow, String> {
        self.executor
            .step(inbound, outbound)
            .map_err(|error| failed(format!("a frame did not decode: {error}")))?;
        let batch = self.wire.take_inbox();
        let (runtime, wire) = (&mut self.runtime, &self.wire);
        let closed = self
            .executor
            .enter(|| runtime.step(batch, woken, wire))
            .map_err(failed)?;
        self.executor.step(Vec::new(), outbound)?;
        Ok(match closed {
            true => Flow::Finished,
            false => Flow::Running,
        })
    }
}

pub(crate) fn initialize_storage(size: usize, align: usize) {
    wasi_threads::initialize_main_thread_storage(size, align);
}

fn failed(error: String) -> String {
    format!("the plugin could not run a frame: {error}")
}
