use block_guest_abi::Flow;
use block_plugin_api::{ErrorCode, Message, ProtocolError, decode_frame, encode_frame};

use crate::{Waker, runtime::Runtime, wasm::surface};

pub struct Editor(Runtime);

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
        surface::initialize().map_err(|error| format!("{name} could not start: {error}"))?;
        post(outbound, vec![runtime.hello()]);
        Ok(Self(runtime))
    }

    pub fn step(
        &mut self,
        inbound: Vec<Vec<u8>>,
        woken: bool,
        outbound: &mut Vec<Vec<u8>>,
    ) -> Result<Flow, String> {
        let mut batch = Vec::with_capacity(inbound.len());
        for frame in inbound {
            batch.push(decode_frame(&frame).map_err(|error| failed(format!("{error:?}")))?);
        }
        match self.0.step(batch, woken) {
            Ok(step) => {
                post(outbound, step.outbound);
                Ok(match step.closed {
                    true => Flow::Finished,
                    false => Flow::Running,
                })
            }
            Err(error) => {
                post(outbound, vec![protocol_error(error.clone())]);
                Err(failed(error))
            }
        }
    }
}

pub(crate) fn initialize_storage(size: usize, align: usize) {
    wasi_threads::initialize_main_thread_storage(size, align);
}

fn failed(error: String) -> String {
    format!("the plugin could not run a frame: {error}")
}

fn post(outbound: &mut Vec<Vec<u8>>, messages: Vec<Message>) {
    for message in messages {
        match encode_frame(&message) {
            Ok(frame) => outbound.push(frame),
            Err(error) => eprintln!("dropped a message the protocol refused to carry: {error}"),
        }
    }
}

fn protocol_error(message: String) -> Message {
    Message::Error(ProtocolError {
        request_id: None,
        code: ErrorCode::Internal,
        message,
    })
}
