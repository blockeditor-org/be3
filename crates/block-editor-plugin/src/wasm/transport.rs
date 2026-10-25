use block_guest_abi::Flow;
use block_plugin_api::{ErrorCode, Message, ProtocolError, ops::guest::Executor};

use super::wire::Shared;
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
            wire.route(vec![runtime.hello()]);
            wire.watch();
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
        let (messages, flow) = match self.runtime.step(batch, woken) {
            Ok(step) => (
                step.outbound,
                match step.closed {
                    true => Flow::Finished,
                    false => Flow::Running,
                },
            ),
            Err(error) => {
                let wire = &self.wire;
                self.executor
                    .enter(|| wire.route(vec![protocol_error(error.clone())]));
                self.executor.step(Vec::new(), outbound)?;
                return Err(failed(error));
            }
        };
        let wire = &self.wire;
        self.executor.enter(|| wire.route(messages));
        self.executor.step(Vec::new(), outbound)?;
        Ok(flow)
    }
}

pub(crate) fn initialize_storage(size: usize, align: usize) {
    wasi_threads::initialize_main_thread_storage(size, align);
}

fn failed(error: String) -> String {
    format!("the plugin could not run a frame: {error}")
}

fn protocol_error(message: String) -> Message {
    Message::Error(ProtocolError {
        request_id: None,
        code: ErrorCode::Internal,
        message,
    })
}
